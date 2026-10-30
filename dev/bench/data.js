window.BENCHMARK_DATA = {
  "lastUpdate": 1793323352950,
  "repoUrl": "https://github.com/WyattAu/aether-core",
  "entries": {
    "Benchmark": [
      {
        "commit": {
          "author": {
            "email": "wyatt_au@protonmail.com",
            "name": "WyattAu",
            "username": "WyattAu"
          },
          "committer": {
            "email": "wyatt_au@protonmail.com",
            "name": "WyattAu",
            "username": "WyattAu"
          },
          "distinct": true,
          "id": "c5c89aa29fa77cb5e36a81fa321441f8bb674b38",
          "message": "fix(core): search-files walked a canonicalized dir but stripped an unresolved root — same macOS /var/folders symlink divergence as resolve_path; thread the canonical root through search_recursive. E2E: skip runtime-bound examples (aether-core dep) in the wasm32-wasip1 build loop — they are host-side programs, not WASM actors",
          "timestamp": "2026-10-30T01:17:30Z",
          "tree_id": "625a68bc265fae80133b89c7d20a2597b8d37739",
          "url": "https://github.com/WyattAu/aether-core/commit/c5c89aa29fa77cb5e36a81fa321441f8bb674b38"
        },
        "date": 1793323352216,
        "tool": "cargo",
        "benches": [
          {
            "name": "registry/register/100",
            "value": 3291388,
            "range": "± 121726",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/1000",
            "value": 60450863,
            "range": "± 17219592",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/10000",
            "value": 621831597,
            "range": "± 215952772",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/100",
            "value": 18,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/1000",
            "value": 18,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/10000",
            "value": 18,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/100",
            "value": 2180,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/1000",
            "value": 18664,
            "range": "± 207",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/10000",
            "value": 184556,
            "range": "± 1414",
            "unit": "ns/iter"
          },
          {
            "name": "registry/concurrent/4_threads_1000_each",
            "value": 301146446,
            "range": "± 170391978",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}