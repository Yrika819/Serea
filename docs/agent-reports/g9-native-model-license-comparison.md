# G9 Preflight — Native Model and Runtime License Evidence

**Checked:** 2026-10-29 UTC
**Source tree reviewed:** `233dbdd4fdb64eeb86b43d48b8e71e5452d9d0a0`
**Tree SHA reviewed:** `515572da542c676c6957a47ad693cd78ee4cb5b3`
**PR:** #144, open Draft
**Scope:** upstream metadata, pinned native build configuration, and source license notices. This is evidence gathering, not legal approval or product distribution clearance.

No GGUF weights were downloaded or committed for this review. The source tree review does not claim model quality or a completed native APK build.

## Findings

| Area | Verified evidence | Status and unresolved points |
|---|---|---|
| Test model source | `ggml-org/SmolLM2-135M-GGUF`, revision `1076b85d135017971845b19b8d07df1479155988`. Its model card declares `apache-2.0` and names `HuggingFaceTB/SmolLM2-135M` as the base model. | **Metadata verified.** The GGUF repository tree has no separate `LICENSE`, `COPYING`, or `NOTICE` file. Its license is a repository/card declaration; no legal opinion on whether the quantized file and all upstream material are cleared for Serea redistribution was made. |
| Model file identity | The pinned revision's Hugging Face tree API identifies `SmolLM2-135M-Q4_K_M.gguf`, reports 101,016,128 bytes, and reports LFS SHA-256 `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5`, matching the workflow's expected values. | **Upstream metadata verified; file bytes not fetched or re-hashed in this audit.** Full byte-integrity verification remains a CI action. Model weights remain outside Git and outside the uploaded evidence path. |
| Base model | At check time the base repository API resolved `HuggingFaceTB/SmolLM2-135M` to `93efa2f097d58c2a74874c7e644dbc9b0cee75a2`; that revision's model card declares Apache-2.0 and links the Apache license text. | **Current base-card declaration verified.** The GGUF card records only the base repository name, not that base revision. This does not establish which exact source revision was converted. The model card also names training datasets; no separate dataset-rights or provenance review was done. |
| llama.cpp / ggml | The runtime pins `ggml-org/llama.cpp` commit `276378515ceebee5d14bcc8536603dd5c87491a2`. Its root `LICENSE` is MIT. The Serea JNI target links `llama`, which links `ggml`; the configured backend is CPU. | **Source license and static link graph verified.** MIT distribution requires preserving the copyright and permission notice in copies or substantial portions. No native NOTICE/license bundle is yet attached to a Serea APK. |
| Native build configuration | The isolated build disables common/tools/examples/tests/server/app, OpenSSL, OpenMP, LLAMAFILE, dynamic backends, and KleidiAI. It links Android `log`/`android`; upstream ggml also declares system thread, `dl`, and math linkage for Android builds. | **Configuration inspected, build not yet evidenced on this source tree.** The final `.so` link map, NDK C++ runtime packaging, and APK contents are not verified until a successful exact-head build and APK inspection. |
| Vendored native source | The pinned upstream CMake always declares the `vendor/hash` target. Its source lists xxHash (BSD-2-Clause), SHA-1 and SHA-256 files marked public domain, and rotate-bits (MIT). Other vendor targets are also declared; `cpp-httplib` is gated by `LLAMA_BUILD_COMMON`, which this build disables. | **Source-tree licenses checked; packaged membership NOT_RUN.** The Serea target graph does not demonstrate which unlinked vendor targets enter the final APK. Include notices for every vendored component shown to be compiled into or distributed with the APK. |
| SBOM / attribution | Serea's lockfile SBOM covers Gradle coordinates, and its APK binder records an APK hash/package identity. Both tools document that they do not establish the native dependency graph, packaged membership, or native artifact hashes. | **Native SBOM and APK NOTICE gap remains open.** The existing Gradle SBOM is not a complete native-runtime SBOM. Add source revision, native ELF/APK hashes, linked native libraries, relevant third-party license texts, and model provenance if model weights ever ship in a product package. |

## Redistribution obligations and decision boundary

The Apache-2.0 text requires redistribution recipients to receive a copy of the license and, where applicable, preservation of copyright, patent, trademark, and attribution notices; modified files must carry a change notice. The pinned GGUF repository exposes Apache-2.0 in its metadata and README but does not include a separate license or NOTICE file. Before redistributing this quantized model, the Owner must decide whether those repository declarations and the base model's terms cover the intended use and packaging, and arrange the required notices. This report does not make that legal determination.

The llama.cpp MIT license requires retaining its copyright and permission notice in copies or substantial portions. The pinned source contains the MIT text, but the opt-in Android APK and app-level notices have not been built and inspected at this head. Separate vendor notices may apply if vendored code is included. Keep model weights out of Git and CI artifacts; the current diagnostic workflow downloads the fixture to runner temporary storage, verifies its expected digest, stages it only in the disposable emulator's app-private storage, then verifies cleanup.

## Evidence status

```text
MODEL_REPOSITORY_LICENSE_METADATA=VERIFIED (upstream declaration only)
MODEL_FILE_SHA256_AND_SIZE=UPSTREAM_METADATA_MATCH; BYTES_NOT_DOWNLOADED_IN_THIS_AUDIT
MODEL_REDISTRIBUTION_CLEARANCE=NOT_ESTABLISHED
LLAMA_CPP_SOURCE_PIN_AND_ROOT_MIT_LICENSE=VERIFIED
NATIVE_APK_COMPONENT_INVENTORY=NOT_RUN
NATIVE_NOTICE_BUNDLE=NOT_RUN
APK_BOUND_NATIVE_SBOM=NOT_RUN
X86_64_PRODUCT_JNI_BUILD_AND_INFERENCE=NOT_RUN on reviewed source tree
ARM64_BUILD=NOT_RUN
ARM64_RUNTIME=NOT_RUN
PIXEL7A_PHYSICAL=NOT_RUN
PRODUCT_CHAT_ROUTE=NOT_CONNECTED
MODEL_QUALITY=NOT_ASSESSED
```

The exact-head focused workflow observed during the G8 checkpoint was queued; it is not used here as build or inference evidence. The candidate remains default-off and isolated from normal Chat.

## Sources checked on 2026-10-29

All model API requests were metadata/tree requests; no model weight URL was fetched.

- GGUF repository metadata at the pinned revision: <https://huggingface.co/api/models/ggml-org/SmolLM2-135M-GGUF/revision/1076b85d135017971845b19b8d07df1479155988>
- GGUF file tree and LFS size/digest at the pinned revision: <https://huggingface.co/api/models/ggml-org/SmolLM2-135M-GGUF/tree/1076b85d135017971845b19b8d07df1479155988?recursive=true&expand=true>
- GGUF model card at that revision: <https://huggingface.co/ggml-org/SmolLM2-135M-GGUF/tree/1076b85d135017971845b19b8d07df1479155988>
- Base model metadata at check time: <https://huggingface.co/api/models/HuggingFaceTB/SmolLM2-135M>
- Base model card at the API-resolved revision: <https://huggingface.co/HuggingFaceTB/SmolLM2-135M/tree/93efa2f097d58c2a74874c7e644dbc9b0cee75a2>
- Apache License 2.0 text: <https://www.apache.org/licenses/LICENSE-2.0>
- llama.cpp pinned root license: <https://github.com/ggml-org/llama.cpp/blob/276378515ceebee5d14bcc8536603dd5c87491a2/LICENSE>
- llama.cpp pinned CMake target graph: <https://github.com/ggml-org/llama.cpp/blob/276378515ceebee5d14bcc8536603dd5c87491a2/src/CMakeLists.txt>, <https://github.com/ggml-org/llama.cpp/blob/276378515ceebee5d14bcc8536603dd5c87491a2/ggml/src/CMakeLists.txt>, <https://github.com/ggml-org/llama.cpp/blob/276378515ceebee5d14bcc8536603dd5c87491a2/vendor/CMakeLists.txt>
- Pinned vendored hash source/license records: <https://github.com/ggml-org/llama.cpp/tree/276378515ceebee5d14bcc8536603dd5c87491a2/vendor/hash>
- Individual vendor license/source notices: [xxHash BSD-2-Clause](https://github.com/ggml-org/llama.cpp/blob/276378515ceebee5d14bcc8536603dd5c87491a2/vendor/hash/xxhash/LICENSE), [SHA-256 public-domain notice](https://github.com/ggml-org/llama.cpp/blob/276378515ceebee5d14bcc8536603dd5c87491a2/vendor/hash/sha256/LICENSE), [rotate-bits MIT](https://github.com/ggml-org/llama.cpp/blob/276378515ceebee5d14bcc8536603dd5c87491a2/vendor/hash/rotate-bits/LICENSE.md), and the [SHA-1 source header](https://github.com/ggml-org/llama.cpp/blob/276378515ceebee5d14bcc8536603dd5c87491a2/vendor/hash/sha1/sha1.c) marked public domain.
- Serea native build flags and link target at reviewed source head: <https://github.com/Yrika819/Serea/blob/233dbdd4fdb64eeb86b43d48b8e71e5452d9d0a0/android/app/src/main/cpp/CMakeLists.txt>, <https://github.com/Yrika819/Serea/blob/233dbdd4fdb64eeb86b43d48b8e71e5452d9d0a0/android/app/build.gradle.kts>
- Serea SBOM scope and limitations at reviewed source head: <https://github.com/Yrika819/Serea/blob/233dbdd4fdb64eeb86b43d48b8e71e5452d9d0a0/tools/generate_android_lockfile_sbom.py>, <https://github.com/Yrika819/Serea/blob/233dbdd4fdb64eeb86b43d48b8e71e5452d9d0a0/tools/bind_android_apk_sbom.py>
