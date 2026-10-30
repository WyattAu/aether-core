window.BENCHMARK_DATA = {
  "lastUpdate": 1793329384402,
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
      },
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
          "id": "99bc7ce2e4ef823f920616b00d765399b67d569d",
          "message": "ci(fdb): run fdbserver natively on the runner — the service container's cluster key can never match a host-side cluster file (introspection returned empty, and the fallback string was doubly invalid: hyphens are illegal in cluster names — 'aether-ci@ip:port' fails fdbcli 7.3.7 parse with 2104, verified locally; 'aether:ci@ip:port' parses). Install foundationdb-server, pin public+listen to 127.0.0.1:4500, daemonize under the foundationdb user. Also: stateful-actor example was #![no_std] while using String/format!/to_string — uncompilable on any target; drop the attribute",
          "timestamp": "2026-10-30T01:46:34Z",
          "tree_id": "1d8b95fc561b787b9e2c8f380b51e464bf42f080",
          "url": "https://github.com/WyattAu/aether-core/commit/99bc7ce2e4ef823f920616b00d765399b67d569d"
        },
        "date": 1793325466513,
        "tool": "cargo",
        "benches": [
          {
            "name": "registry/register/100",
            "value": 7053343,
            "range": "± 185928",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/1000",
            "value": 87759163,
            "range": "± 115480140",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/10000",
            "value": 878005300,
            "range": "± 247775374",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/100",
            "value": 24,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/1000",
            "value": 24,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/10000",
            "value": 24,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/100",
            "value": 2403,
            "range": "± 14",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/1000",
            "value": 21096,
            "range": "± 620",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/10000",
            "value": 208128,
            "range": "± 452",
            "unit": "ns/iter"
          },
          {
            "name": "registry/concurrent/4_threads_1000_each",
            "value": 430398927,
            "range": "± 79353640",
            "unit": "ns/iter"
          }
        ]
      },
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
          "id": "ffd58fc8f0a5ba7e0c4447fe67f1182b5149151c",
          "message": "fix(examples): stateful-actor rebuilt against the real SDK API — the old source was written against an imagined API (#[aether_actor] attribute, async Handler::handle, String error type) and could not compile for any target; rewrite on Handler::handle(sync, ActorResult) + StateHandle bridged with futures::executor, export_actor!(StatefulActor::new()) — wasm32-wasip1 build verified locally with the CI command",
          "timestamp": "2026-10-30T01:59:47Z",
          "tree_id": "0eaf86916e616d569a7573e2dccbd37ae0d7f6d3",
          "url": "https://github.com/WyattAu/aether-core/commit/ffd58fc8f0a5ba7e0c4447fe67f1182b5149151c"
        },
        "date": 1793326200238,
        "tool": "cargo",
        "benches": [
          {
            "name": "registry/register/100",
            "value": 6684084,
            "range": "± 434044",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/1000",
            "value": 88122375,
            "range": "± 6989306",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/10000",
            "value": 975177530,
            "range": "± 234611978",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/100",
            "value": 24,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/1000",
            "value": 24,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/10000",
            "value": 24,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/100",
            "value": 2401,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/1000",
            "value": 20993,
            "range": "± 115",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/10000",
            "value": 207195,
            "range": "± 509",
            "unit": "ns/iter"
          },
          {
            "name": "registry/concurrent/4_threads_1000_each",
            "value": 369431248,
            "range": "± 108435514",
            "unit": "ns/iter"
          }
        ]
      },
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
          "id": "9c6275c21c051f7c5c5cf882f8c33b98a736e9fc",
          "message": "ci(fdb): fdbserver 7.3 has --daemon, not --daemonize (unknown option at startup)",
          "timestamp": "2026-10-30T02:33:01Z",
          "tree_id": "4632558950f27da1b666ffd847e3a3bbb9ed95c8",
          "url": "https://github.com/WyattAu/aether-core/commit/9c6275c21c051f7c5c5cf882f8c33b98a736e9fc"
        },
        "date": 1793327991833,
        "tool": "cargo",
        "benches": [
          {
            "name": "registry/register/100",
            "value": 11717165,
            "range": "± 337266",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/1000",
            "value": 107742885,
            "range": "± 89711467",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/10000",
            "value": 1154789998,
            "range": "± 267515597",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/100",
            "value": 24,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/1000",
            "value": 24,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/10000",
            "value": 24,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/100",
            "value": 2407,
            "range": "± 8",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/1000",
            "value": 20978,
            "range": "± 48",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/10000",
            "value": 206708,
            "range": "± 391",
            "unit": "ns/iter"
          },
          {
            "name": "registry/concurrent/4_threads_1000_each",
            "value": 333662933,
            "range": "± 122404332",
            "unit": "ns/iter"
          }
        ]
      },
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
          "id": "666133a16b817ac1dfb9269df05c038096c32a93",
          "message": "ci(fdb): pin public+listen to 127.0.0.1:4500 — auto: advertises the primary interface IP which never matches the cluster file's coordinator address",
          "timestamp": "2026-10-30T02:58:29Z",
          "tree_id": "ab3d489f9320d6c64c84d1fc8b50df12c70e2201",
          "url": "https://github.com/WyattAu/aether-core/commit/666133a16b817ac1dfb9269df05c038096c32a93"
        },
        "date": 1793329383359,
        "tool": "cargo",
        "benches": [
          {
            "name": "registry/register/100",
            "value": 3068114,
            "range": "± 162698",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/1000",
            "value": 53334133,
            "range": "± 94387285",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/10000",
            "value": 560430703,
            "range": "± 154204224",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/100",
            "value": 25,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/1000",
            "value": 25,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/10000",
            "value": 25,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/100",
            "value": 2567,
            "range": "± 28",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/1000",
            "value": 19267,
            "range": "± 526",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/10000",
            "value": 186880,
            "range": "± 2849",
            "unit": "ns/iter"
          },
          {
            "name": "registry/concurrent/4_threads_1000_each",
            "value": 234125483,
            "range": "± 83695706",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}