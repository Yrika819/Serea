# G10-B1 Native supply-chain evidence

- Upstream: https://github.com/ggml-org/llama.cpp
- Pinned source: `276378515ceebee5d14bcc8536603dd5c87491a2`
- Retrieved directly from the upstream Git remote on 2026-11-07; exact commit checkout succeeded and the worktree was clean.
- Upstream `LICENSE` is MIT, copyright ggml authors (2023-2026); its SHA-256 is `94f29bbed6a22c35b992c5c6ebf0e7c92f13b836b90f36f461c9cf2f0f1d010d`.
- Source tree has no git submodule entries at the pinned revision. The vendored tree contains separate MIT notices for nlohmann/json and cpp-httplib. The current CMake configuration disables `LLAMA_BUILD_COMMON`, tools, examples, server, OpenSSL, tests and dynamically loaded backends; actual linked dependency inclusion remains subject to the isolated build's link report.
- No model weights are stored in the repository or workflow artifact.
- Native source SHA-256 at HEAD `5a9831c4da171aecddddb0a78c282c85a6248131`: `8bb10023199e8cd40b7310c8c5a423b50c91ad3fc9da723382cd2106b4a4e649` (`llama_jni.cpp`); CMake SHA-256: `88c27fb4d4aa315248eeed8cfae2d3f1cda8b6bbd39f038e612984f710024302`.
- Android shared library and APK hashes are emitted by `.github/workflows/g10-b1-native.yml` after a successful per-ABI isolated build. Until that workflow passes, ABI and packaged dependency provenance are NOT_RUN.
