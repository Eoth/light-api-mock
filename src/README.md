# src/ — Backend Rust

## Modules

### `models/`
Types serialisables (Serde) :
- `Service` : name (= namespace URL), method, listen_path, real_target_url, is_mocked, rules
- `Rule`, `ConditionGroup`, `Condition`
- `ConditionSource` : QueryParam, Header, PathParam, JsonPointer, XPath, FormField, BodyRaw
- `Operator` : Eq, Contains, Regex, Exists
- `BodyFragment` : Literal, Template, Uuid, PickFrom, FakeData, PathSegment
- `FakeKind` : FirstName, LastName, Email, PhoneNumberFR, CompanyName, Siren, Siret, CityFR, PostcodeFR, StreetName, FullAddressFR, DatePast, DateFuture, TimestampMs, Integer
- `ChaosConfig` : delay_ms, delay_min_ms/delay_max_ms, error_rate, error_status

### `engine/`
- `matcher.rs` : evaluation first-match, extraction depuis path params, query, headers, JSON, XML, form, body
- `proxy.rs` : reverse proxy (hop-by-hop filtering, X-Forwarded-Host/Proto)
- `renderer.rs` : rendu des fragments + fake data generators + chaos mode
- `template.rs` : parser d'expressions `{path.siret | first(9)}`, variables, pipes

### `store/`
- `MockStore` : `Arc<RwLock<MockConfig>>`, ecriture atomique temp+rename, chemin via DATA_PATH

### `server/`
- `intercept.rs` : middleware — namespace URL `/{name}/...`, filtre methode HTTP, matching, logs enrichis
- `api.rs` : CRUD REST + GET /api/logs
- `request_log.rs` : ring buffer 200 entrees (LogEntry), helpers log_mock/log_proxy/log_no_rule
- `mod.rs` : AppState (store, proxy, seq_counters, request_log), build_router

## Tests

```bash
cargo test -- --test-threads=1
```
