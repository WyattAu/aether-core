window.BENCHMARK_DATA = {
  "lastUpdate": 1793347136398,
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
          "id": "e1a62254e1d2fda81cae387d08bc777cef85867f",
          "message": "ci(fdb): fdbserver 7.3 has no daemon flag at all (-d is --datadir — it consumed the next arg); detach with nohup+& . Full chain rehearsed locally: pinned-address server + aether:ci cluster file + configure new single memory + status minimal all green against fdbcli/fdbserver 7.3.27",
          "timestamp": "2026-10-30T03:23:03Z",
          "tree_id": "bdb0c037d10ffa20a685c8d54baa7fc267253a66",
          "url": "https://github.com/WyattAu/aether-core/commit/e1a62254e1d2fda81cae387d08bc777cef85867f"
        },
        "date": 1793330997916,
        "tool": "cargo",
        "benches": [
          {
            "name": "registry/register/100",
            "value": 9854189,
            "range": "± 565011",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/1000",
            "value": 129545320,
            "range": "± 28139040",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/10000",
            "value": 1307790020,
            "range": "± 185532953",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/100",
            "value": 35,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/1000",
            "value": 35,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/10000",
            "value": 35,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/100",
            "value": 3546,
            "range": "± 37",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/1000",
            "value": 28730,
            "range": "± 787",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/10000",
            "value": 278222,
            "range": "± 1908",
            "unit": "ns/iter"
          },
          {
            "name": "registry/concurrent/4_threads_1000_each",
            "value": 366309853,
            "range": "± 96876832",
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
          "id": "5fded7a5626f0b819e61b3b431b2b9911be9e1c7",
          "message": "ci(fdb): bound every fdbcli call with timeout 20 — the wait loop hung the job 50m on a blocking call; loop now surfaces the server process, stdout log, and cluster file on failure",
          "timestamp": "2026-10-30T04:14:29Z",
          "tree_id": "ce5ae58ca8bc589c99433ff0b5739726b8b13aae",
          "url": "https://github.com/WyattAu/aether-core/commit/5fded7a5626f0b819e61b3b431b2b9911be9e1c7"
        },
        "date": 1793333947064,
        "tool": "cargo",
        "benches": [
          {
            "name": "registry/register/100",
            "value": 3230423,
            "range": "± 319074",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/1000",
            "value": 50851782,
            "range": "± 96793327",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/10000",
            "value": 602142517,
            "range": "± 182352744",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/100",
            "value": 25,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/1000",
            "value": 25,
            "range": "± 0",
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
            "value": 2555,
            "range": "± 38",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/1000",
            "value": 19016,
            "range": "± 128",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/10000",
            "value": 187497,
            "range": "± 3801",
            "unit": "ns/iter"
          },
          {
            "name": "registry/concurrent/4_threads_1000_each",
            "value": 252284995,
            "range": "± 23504505",
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
          "id": "98270a422355fba87b638c1a95ca3947c62dd459",
          "message": "ci(fdb): stop fighting the server .deb — its postinst generates the cluster file AND starts a matching fdbserver via fdbmonitor; rewriting the file desynced it from the running server ('Unable to locate a cluster controller', visible in the wait-loop diagnostics). Use the generated pair as-is; the wait loop's configure fallback creates the database",
          "timestamp": "2026-10-30T04:43:10Z",
          "tree_id": "7c251b368d1580d41a0d51206a7d552ceb4e7121",
          "url": "https://github.com/WyattAu/aether-core/commit/98270a422355fba87b638c1a95ca3947c62dd459"
        },
        "date": 1793335808895,
        "tool": "cargo",
        "benches": [
          {
            "name": "registry/register/100",
            "value": 15535330,
            "range": "± 614209",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/1000",
            "value": 112842543,
            "range": "± 104747869",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/10000",
            "value": 1171959122,
            "range": "± 236029046",
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
            "value": 2396,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/1000",
            "value": 20997,
            "range": "± 40",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/10000",
            "value": 207740,
            "range": "± 826",
            "unit": "ns/iter"
          },
          {
            "name": "registry/concurrent/4_threads_1000_each",
            "value": 431230848,
            "range": "± 80996124",
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
          "id": "52d9e13fe16f96be61289cfd2a7b4e16cabacb1c",
          "message": "fix(core): boot the FDB client network before any API use — foundationdb 0.9 requires exactly one process-lifetime boot(); every client call failed with 'Action not possible before the network is configured' (first live run of the FDB integration suite). Verified locally: the network error is gone; the residual Unhealthy is the absent local server",
          "timestamp": "2026-10-30T05:08:43Z",
          "tree_id": "76e71de260db8b3d1fff67de01e8d40c7a321b29",
          "url": "https://github.com/WyattAu/aether-core/commit/52d9e13fe16f96be61289cfd2a7b4e16cabacb1c"
        },
        "date": 1793337317752,
        "tool": "cargo",
        "benches": [
          {
            "name": "registry/register/100",
            "value": 7301526,
            "range": "± 207326",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/1000",
            "value": 94049311,
            "range": "± 114377577",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/10000",
            "value": 1045290485,
            "range": "± 293157986",
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
            "value": 2386,
            "range": "± 8",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/1000",
            "value": 20990,
            "range": "± 43",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/10000",
            "value": 206725,
            "range": "± 860",
            "unit": "ns/iter"
          },
          {
            "name": "registry/concurrent/4_threads_1000_each",
            "value": 452898861,
            "range": "± 304966701",
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
          "id": "2eb6987d244aa58031817c90bea7547fef760832",
          "message": "test(fdb): range-scan keys must carry the scanned prefix — format!(\"{:?}\", (prefix, i)) wrote Debug-tuple strings ('(\"test_range_\", 0)') outside the scan range, so the assertion saw 0 results; zero-padded real keys keep lexicographic order deterministic. First run of this suite ever — the test predated the server it needs",
          "timestamp": "2026-10-30T05:32:12Z",
          "tree_id": "8439d26798c13eda18b7deac4cd8b71c936f4c50",
          "url": "https://github.com/WyattAu/aether-core/commit/2eb6987d244aa58031817c90bea7547fef760832"
        },
        "date": 1793338757166,
        "tool": "cargo",
        "benches": [
          {
            "name": "registry/register/100",
            "value": 3127170,
            "range": "± 146189",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/1000",
            "value": 51202974,
            "range": "± 106835222",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/10000",
            "value": 579495550,
            "range": "± 189252485",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/100",
            "value": 25,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/1000",
            "value": 25,
            "range": "± 0",
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
            "value": 2591,
            "range": "± 56",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/1000",
            "value": 19338,
            "range": "± 573",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/10000",
            "value": 186085,
            "range": "± 2593",
            "unit": "ns/iter"
          },
          {
            "name": "registry/concurrent/4_threads_1000_each",
            "value": 258618732,
            "range": "± 53316823",
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
          "id": "04dc66a7418b043cd8ae745f4cbc1ea96376d6a2",
          "message": "ci(docker): the compose integration job needs more than 30 minutes on a cold cache (cancelled at the budget with all steps green so far); 60 minutes",
          "timestamp": "2026-10-30T06:28:42Z",
          "tree_id": "235618c19f21de5743a5ba329a1b459febea6c1b",
          "url": "https://github.com/WyattAu/aether-core/commit/04dc66a7418b043cd8ae745f4cbc1ea96376d6a2"
        },
        "date": 1793342036047,
        "tool": "cargo",
        "benches": [
          {
            "name": "registry/register/100",
            "value": 3655721,
            "range": "± 213770",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/1000",
            "value": 97455801,
            "range": "± 4353557",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/10000",
            "value": 628326162,
            "range": "± 221184134",
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
            "value": 2190,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/1000",
            "value": 18874,
            "range": "± 53",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/10000",
            "value": 183903,
            "range": "± 3685",
            "unit": "ns/iter"
          },
          {
            "name": "registry/concurrent/4_threads_1000_each",
            "value": 316134615,
            "range": "± 76731017",
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
          "id": "10a66d833aa119472e817b84caaea8fc804b9f09",
          "message": "ci(docker): a fresh FDB container has no database — poll status AND attempt 'configure new single memory' until 'The database is available'; the old check demanded availability from a cluster that was never configured (36 minutes of futile retries)",
          "timestamp": "2026-10-30T07:09:48Z",
          "tree_id": "1f4385705b519d89759c6d5978002c7007801509",
          "url": "https://github.com/WyattAu/aether-core/commit/10a66d833aa119472e817b84caaea8fc804b9f09"
        },
        "date": 1793344785837,
        "tool": "cargo",
        "benches": [
          {
            "name": "registry/register/100",
            "value": 7217320,
            "range": "± 267479",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/1000",
            "value": 101921782,
            "range": "± 109984536",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/10000",
            "value": 987118635,
            "range": "± 210920156",
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
            "value": 2429,
            "range": "± 8",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/1000",
            "value": 21012,
            "range": "± 34",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/10000",
            "value": 207022,
            "range": "± 324",
            "unit": "ns/iter"
          },
          {
            "name": "registry/concurrent/4_threads_1000_each",
            "value": 422208136,
            "range": "± 164672584",
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
          "id": "a40d9d4d6618897ecfd57b5050afc341d2801b10",
          "message": "ci(docker): drop FDB_NETWORKING_MODE=host (binds fdbserver to the container's loopback — unreachable through -p 4500:4500); extract locates the cluster file from fdbserver's cmdline and rewrites the coordinator IP to 127.0.0.1 while preserving the name:desc identity, matching ci.yml's working pattern",
          "timestamp": "2026-10-30T07:28:45Z",
          "tree_id": "1e88a88023592fadbb05a0ef5878b697dd531e82",
          "url": "https://github.com/WyattAu/aether-core/commit/a40d9d4d6618897ecfd57b5050afc341d2801b10"
        },
        "date": 1793345732833,
        "tool": "cargo",
        "benches": [
          {
            "name": "registry/register/100",
            "value": 11099041,
            "range": "± 160606",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/1000",
            "value": 167926815,
            "range": "± 1408728",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/10000",
            "value": 1271802155,
            "range": "± 151311293",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/100",
            "value": 32,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/1000",
            "value": 32,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/10000",
            "value": 32,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/100",
            "value": 3157,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/1000",
            "value": 25819,
            "range": "± 42",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/10000",
            "value": 248656,
            "range": "± 352",
            "unit": "ns/iter"
          },
          {
            "name": "registry/concurrent/4_threads_1000_each",
            "value": 362285262,
            "range": "± 78218426",
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
          "id": "436300b2b3f2126f48dbcd650af0f6b79edf54f2",
          "message": "ci(docker): pin the compose build context to the absolute repo root — compose v2 resolved 'context: ../..' to <repo>/tests (one level short), so buildkit lstat'd <repo>/tests/tests inside COPY tests and died before building. Interpolate ${AETHER_REPO_ROOT} from github.workspace; ../.. remains the local fallback",
          "timestamp": "2026-10-30T07:48:48Z",
          "tree_id": "b55fd57d6ead3d935f524ad9f4b96f2104390f12",
          "url": "https://github.com/WyattAu/aether-core/commit/436300b2b3f2126f48dbcd650af0f6b79edf54f2"
        },
        "date": 1793347135347,
        "tool": "cargo",
        "benches": [
          {
            "name": "registry/register/100",
            "value": 10766665,
            "range": "± 262591",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/1000",
            "value": 237874939,
            "range": "± 43547453",
            "unit": "ns/iter"
          },
          {
            "name": "registry/register/10000",
            "value": 1263987595,
            "range": "± 193145945",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/100",
            "value": 34,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/1000",
            "value": 34,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "registry/lookup/10000",
            "value": 34,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/100",
            "value": 3416,
            "range": "± 15",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/1000",
            "value": 27566,
            "range": "± 163",
            "unit": "ns/iter"
          },
          {
            "name": "registry/list/10000",
            "value": 265652,
            "range": "± 1140",
            "unit": "ns/iter"
          },
          {
            "name": "registry/concurrent/4_threads_1000_each",
            "value": 410506850,
            "range": "± 47628386",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}