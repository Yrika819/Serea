//! Reproducible synthetic Japanese lexical-search comparison. This is a
//! benchmark harness, not a quality threshold or production tokenizer.
use rusqlite::{Connection, params};
use std::time::Instant;

const CORPUS: &[(&str, &str)] = &[
    (
        "m01",
        "今日は東京で会議です。議題はSerea Memoryの設計です。",
    ),
    ("m02", "東京駅で待ち合わせ。Meeting starts at 9:30."),
    ("m03", "明日の15時に佐藤さんへメールを送る。"),
    ("m04", "プロジェクト Serea の P7 レビューは金曜日です。"),
    ("m05", "週末に牛乳と卵を買う。"),
    ("m06", "Alice will review the memory schema on October 12."),
    ("m07", "渋谷のカフェでコーヒーを飲む。"),
    ("m08", "会議は延期、次回は来週月曜日。"),
    ("m09", "APIのtimeoutを3秒に設定する。"),
    ("m10", "横浜で家族と昼食。予約名は田中です。"),
    ("m11", "今日の買い物: パン、牛乳、りんご。"),
    ("m12", "Serea memory retrieval test uses SQLite FTS5."),
];

const QUERIES: &[(&str, &[&str])] = &[
    ("東京", &["m01", "m02"]),
    ("会議", &["m01", "m08"]),
    ("佐藤", &["m03"]),
    ("15時", &["m03"]),
    ("金曜日", &["m04"]),
    ("牛乳", &["m05", "m11"]),
    ("Serea", &["m01", "m04", "m12"]),
    ("October 12", &["m06"]),
    ("timeout", &["m09"]),
    ("田中", &["m10"]),
    ("memory retrieval", &["m12"]),
];

#[derive(Debug)]
struct Metrics {
    version: String,
    exact_answer_queries: usize,
    complete_recall_queries: usize,
    expected_query_count: usize,
    relevant_found: usize,
    relevant_total: usize,
    false_positive_rows: usize,
    mean_query_us: u128,
    incremental_index_page_bytes: usize,
    rebuild_us: u128,
}

fn host_ja_v1(text: &str) -> String {
    let mut output = Vec::new();
    let mut japanese = Vec::new();
    let flush_japanese = |run: &mut Vec<char>, out: &mut Vec<String>| {
        if run.len() == 1 {
            out.push(run[0].to_string());
        } else {
            for pair in run.windows(2) {
                out.push(pair.iter().collect());
            }
        }
        run.clear();
    };
    for ch in text.chars() {
        if ('\u{3040}'..='\u{30ff}').contains(&ch) || ('\u{3400}'..='\u{9fff}').contains(&ch) {
            japanese.push(ch);
        } else {
            flush_japanese(&mut japanese, &mut output);
            if ch.is_ascii_alphanumeric() {
                output.push(ch.to_ascii_lowercase().to_string());
            }
        }
    }
    flush_japanese(&mut japanese, &mut output);
    output.join(" ")
}

fn build(strategy: &str) -> (Connection, usize) {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE corpus(id TEXT PRIMARY KEY, body TEXT NOT NULL);")
        .unwrap();
    for (id, body) in CORPUS {
        db.execute("INSERT INTO corpus VALUES(?1,?2)", params![id, body])
            .unwrap();
    }
    let baseline_bytes = page_count(&db);
    match strategy {
        "unicode61" => db.execute_batch("CREATE VIRTUAL TABLE search USING fts5(id UNINDEXED, body, original_body UNINDEXED, tokenize='unicode61'); INSERT INTO search SELECT id,body,body FROM corpus;").unwrap(),
        "trigram" => db.execute_batch("CREATE VIRTUAL TABLE search USING fts5(id UNINDEXED, body, original_body UNINDEXED, tokenize='trigram'); INSERT INTO search SELECT id,body,body FROM corpus;").unwrap(),
        "host-ja-v1" => {
            db.execute_batch("CREATE VIRTUAL TABLE search USING fts5(id UNINDEXED, body, original_body UNINDEXED, tokenize='unicode61');").unwrap();
            for (id, body) in CORPUS {
                db.execute("INSERT INTO search VALUES(?1,?2,?3)", params![id, host_ja_v1(body),body]).unwrap();
            }
        }
        _ => unreachable!(),
    }
    (db, baseline_bytes)
}

fn literal_query(strategy: &str, query: &str) -> String {
    let source = if strategy == "host-ja-v1" {
        host_ja_v1(query)
    } else {
        query.to_owned()
    };
    let terms: Vec<&str> = source
        .split_whitespace()
        .filter(|term| !term.is_empty())
        .collect();
    terms
        .iter()
        .map(|term| format!("\"{}\"", term.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn page_count(db: &Connection) -> usize {
    let pages: i64 = db.query_row("PRAGMA page_count", [], |r| r.get(0)).unwrap();
    let page_size: i64 = db.query_row("PRAGMA page_size", [], |r| r.get(0)).unwrap();
    (pages * page_size) as usize
}

// Monotonic wall time is used only to report benchmark observations. It never
// influences protocol, storage, retention, or test pass/fail behavior.
#[allow(clippy::disallowed_methods)]
fn benchmark_instant() -> Instant {
    Instant::now()
}

fn fts_shadow_page_bytes(db: &Connection) -> usize {
    db.query_row(
        "SELECT COALESCE(sum(pgsize),0) FROM dbstat WHERE name GLOB 'search_*'",
        [],
        |row| row.get::<_, i64>(0),
    )
    .unwrap_or(0) as usize
}

fn evaluate(strategy: &str) -> Metrics {
    let version: String = Connection::open_in_memory()
        .unwrap()
        .query_row("SELECT sqlite_version()", [], |r| r.get(0))
        .unwrap();
    let (db, _baseline_bytes) = build(strategy);
    let incremental_index_page_bytes = fts_shadow_page_bytes(&db);
    let started = benchmark_instant();
    db.execute_batch("DROP TABLE search;").unwrap();
    let (rebuilt, _) = build(strategy);
    let rebuild_us = started.elapsed().as_micros();
    let mut exact_answer_queries = 0;
    let mut complete_recall_queries = 0;
    let mut relevant_found = 0;
    let mut relevant_total = 0;
    let mut false_positive_rows = 0;
    let query_start = benchmark_instant();
    for _ in 0..100 {
        for (query, expected) in QUERIES {
            let expression = literal_query(strategy, query);
            let mut statement = rebuilt
                .prepare("SELECT id,original_body FROM search WHERE search MATCH ?1 ORDER BY id")
                .unwrap();
            let rows = statement
                .query_map([expression], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .unwrap();
            let candidates: Vec<(String, String)> = rows.map(Result::unwrap).collect();
            let mut expected: Vec<String> = expected.iter().map(|id| (*id).to_owned()).collect();
            expected.sort();
            false_positive_rows += candidates
                .iter()
                .filter(|(id, _)| !expected.contains(id))
                .count();
            let actual: Vec<String> = candidates
                .into_iter()
                .filter(|(_, body)| body.contains(query))
                .map(|(id, _)| id)
                .collect();
            relevant_total += expected.len();
            let found = actual.iter().filter(|id| expected.contains(id)).count();
            relevant_found += found;
            if found == expected.len() {
                complete_recall_queries += 1;
            }
            if actual == expected {
                exact_answer_queries += 1;
            }
        }
    }
    let mean_query_us = query_start.elapsed().as_micros() / (QUERIES.len() as u128 * 100);
    Metrics {
        version,
        exact_answer_queries,
        complete_recall_queries,
        expected_query_count: QUERIES.len() * 100,
        relevant_found,
        relevant_total,
        false_positive_rows,
        mean_query_us,
        incremental_index_page_bytes,
        rebuild_us,
    }
}

#[test]
fn synthetic_japanese_retrieval_comparison_records_observed_metrics() {
    for strategy in ["unicode61", "trigram", "host-ja-v1"] {
        let result = evaluate(strategy);
        println!(
            "strategy={strategy} sqlite={} exact_answer_queries={}/{} complete_recall_queries={}/{} relevant_recall={}/{} false_positive_rows={} mean_query_us={} incremental_index_page_bytes={} rebuild_us={}",
            result.version,
            result.exact_answer_queries,
            result.expected_query_count,
            result.complete_recall_queries,
            result.expected_query_count,
            result.relevant_found,
            result.relevant_total,
            result.false_positive_rows,
            result.mean_query_us,
            result.incremental_index_page_bytes,
            result.rebuild_us
        );
        assert_eq!(result.expected_query_count, QUERIES.len() * 100);
    }
}
