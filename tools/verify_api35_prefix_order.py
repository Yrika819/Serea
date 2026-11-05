#!/usr/bin/env python3
import re
import sys
from pathlib import Path
expected = Path(sys.argv[1]).read_text().splitlines()
log = Path(sys.argv[2]).read_text(errors='replace')
events = re.findall(r'SereaTestLifecycle:\s+(START|FINISH|FAIL|RUN_FINISH)\|[0-9]+\|([^\r\n]+)', log)
starts = [v for k, v in events if k == 'START']
finishes = [v for k, v in events if k == 'FINISH']
failures = [v for k, v in events if k == 'FAIL']
run_ends = [v for k, v in events if k == 'RUN_FINISH']
print('expected_count=', len(expected), 'start_count=', len(starts), 'finish_count=', len(finishes), 'fail_count=', len(failures), 'run_finish=', run_ends)
for index, (want, actual) in enumerate(zip(expected, starts), 1):
    if want != actual:
        print(f'first_start_order_difference_at={index} expected={want} actual={actual}')
        break
print('observed_start_order:')
for index, value in enumerate(starts, 1):
    print(f'{index:03d} {value}')
ok = len(expected) == 62 and starts == expected and finishes == expected and not failures and run_ends == ['62|0|0']
print('order_status=' + ('PASS' if ok else 'FAIL'))
raise SystemExit(0 if ok else 1)
