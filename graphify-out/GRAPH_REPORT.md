# Graph Report - .  (2026-06-30)

## Corpus Check
- Corpus is ~4,887 words - fits in a single context window. You may not need a graph.

## Summary
- 120 nodes · 199 edges · 8 communities
- Extraction: 84% EXTRACTED · 16% INFERRED · 0% AMBIGUOUS · INFERRED: 31 edges (avg confidence: 0.81)
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- [[_COMMUNITY_Server Admin API|Server Admin API]]
- [[_COMMUNITY_Configuration System|Configuration System]]
- [[_COMMUNITY_Project Concepts|Project Concepts]]
- [[_COMMUNITY_Proxy Client|Proxy Client]]
- [[_COMMUNITY_Frontend UI|Frontend UI]]
- [[_COMMUNITY_SSE Response Transform|SSE Response Transform]]
- [[_COMMUNITY_Model Registry Data|Model Registry Data]]
- [[_COMMUNITY_CLI Commands|CLI Commands]]

## God Nodes (most connected - your core abstractions)
1. `loadModels()` - 8 edges
2. `NIMPipe` - 8 edges
3. `api()` - 7 edges
4. `ensure_dirs()` - 7 edges
5. `load_config()` - 7 edges
6. `json_response()` - 7 edges
7. `server_main()` - 6 edges
8. `ProxyClient` - 5 edges
9. `ModelRegistry` - 5 edges
10. `load_registry()` - 5 edges

## Surprising Connections (you probably didn't know these)
- `NIMPipe` --references--> `Mission Control UI`  [EXTRACTED]
  README.md → assets/ui/index.html
- `Recent Events Panel` --references--> `NIMPipe`  [EXTRACTED]
  assets/ui/index.html → README.md
- `Test Model Feature` --references--> `OpenAI-Compatible API`  [INFERRED]
  assets/ui/index.html → README.md
- `Model Management UI` --references--> `Model Registry`  [INFERRED]
  assets/ui/index.html → README.md
- `admin_reload()` --calls--> `load_config()`  [INFERRED]
  src/server.rs → src/config.rs

## Hyperedges (group relationships)
- **Model Parameter Normalization** — readme_thinking_variants, readme_chat_template_kwargs, readme_kimi_k2_6, readme_minimax_m3 [INFERRED 0.85]

## Communities (8 total, 0 thin omitted)

### Community 0 - "Server Admin API"
Cohesion: 0.12
Nodes (18): load_registry(), admin_events(), admin_get_config(), admin_list_models(), admin_reload(), admin_test_model(), admin_update_models(), ApiResponse (+10 more)

### Community 1 - "Configuration System"
Cohesion: 0.17
Nodes (16): AppConfig, config_dir(), config_path(), data_dir(), ensure_dirs(), load_config(), load_raw_models(), log_dir() (+8 more)

### Community 2 - "Project Concepts"
Cohesion: 0.15
Nodes (16): NIMPipe, Async 202 Polling, chat_template_kwargs, moonshotai/kimi-k2.6, minimaxai/minimax-m3, Model Registry, NVIDIA NIM, OpenAI-Compatible API (+8 more)

### Community 3 - "Proxy Client"
Cohesion: 0.2
Nodes (10): server_main(), openai_error_response(), ProxyClient, create_app(), serve_asset(), maps_model_alias_and_injects_nested_param(), transform_stream(), sample_model() (+2 more)

### Community 4 - "Frontend UI"
Cohesion: 0.28
Nodes (15): api(), editor, escapeHtml(), init(), loadEvents(), loadModels(), loadStatus(), models (+7 more)

### Community 5 - "SSE Response Transform"
Cohesion: 0.25
Nodes (11): find_sse_event_end(), SseTransformStream, test_transform_sse_data_leaves_content_alone(), test_transform_sse_data_maps_reasoning_to_content(), test_transform_sse_data_null_content(), test_transform_sse_event_preserves_non_json_comments(), test_transform_sse_event_strips_comments(), transform_sse_data() (+3 more)

### Community 6 - "Model Registry Data"
Cohesion: 0.22
Nodes (5): save_raw_models(), ModelEntry, ModelRegistry, ModelRegistryFile, save_registry()

### Community 7 - "CLI Commands"
Cohesion: 0.4
Nodes (4): Cli, Commands, run_command(), ServiceAction

## Knowledge Gaps
- **23 isolated node(s):** `models`, `editor`, `ServerConfig`, `TimeoutsConfig`, `LoggingConfig` (+18 more)
  These have ≤1 connection - possible missing edges or undocumented components.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `transform_stream()` connect `Proxy Client` to `SSE Response Transform`?**
  _High betweenness centrality (0.103) - this node is a cross-community bridge._
- **Why does `server_main()` connect `Proxy Client` to `Server Admin API`, `Configuration System`, `CLI Commands`?**
  _High betweenness centrality (0.100) - this node is a cross-community bridge._
- **Why does `load_config()` connect `Configuration System` to `Server Admin API`, `Proxy Client`?**
  _High betweenness centrality (0.073) - this node is a cross-community bridge._
- **Are the 3 inferred relationships involving `load_config()` (e.g. with `admin_reload()` and `main()`) actually correct?**
  _`load_config()` has 3 INFERRED edges - model-reasoned connections that need verification._
- **What connects `models`, `editor`, `ServerConfig` to the rest of the system?**
  _23 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Server Admin API` be split into smaller, more focused modules?**
  _Cohesion score 0.12 - nodes in this community are weakly interconnected._