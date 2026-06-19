$ErrorActionPreference = "Stop"
$base = "http://localhost:3000"
$pass = 0
$fail = 0

function Test($name, $expected, $actual) {
    if ($actual -match $expected) {
        Write-Host "  PASS: $name" -ForegroundColor Green
        $script:pass++
    } else {
        Write-Host "  FAIL: $name (expected='$expected', got='$actual')" -ForegroundColor Red
        $script:fail++
    }
}

function Api($method, $path, $body) {
    $opts = @{
        Method = $method
        Uri = "$base$path"
        ContentType = "application/json"
        UseBasicParsing = $true
        TimeoutSec = 5
    }
    if ($body) { $opts.Body = $body }
    try {
        $r = Invoke-WebRequest @opts
        return @{ Status = $r.StatusCode; Body = $r.Content }
    } catch {
        $code = [int]$_.Exception.Response.StatusCode
        return @{ Status = $code; Body = "" }
    }
}

Write-Host "`n=== SMOKE TEST lightMock ===" -ForegroundColor Cyan

# 1. Config vide au demarrage
Write-Host "`n[1] Config initiale vide"
$r = Api "GET" "/api/config"
Test "GET /api/config returns 200" "200" $r.Status
Test "Config has empty services" "services.*\[\]" $r.Body

# 2. Creer un service avec mock actif + une regle
Write-Host "`n[2] Creer un service mock"
$svc = @'
{
  "name": "demo-api",
  "listen_path": "/demo-api/*",
  "real_target_url": "http://httpbin.org",
  "is_mocked": true,
  "rewrite_directory_urls": false,
  "rules": [
    {
      "name": "hello-rule",
      "conditions": {
        "all_of": [
          { "source": { "type": "QueryParam", "key": "name" }, "operator": { "type": "Eq", "value": "alice" } }
        ],
        "any_of": []
      },
      "response": {
        "status": 200,
        "headers": [{ "name": "Content-Type", "value": "application/json" }],
        "body": [
          { "type": "Literal", "value": "{\"greeting\":\"Hello " },
          { "type": "Literal", "value": "Alice" },
          { "type": "Literal", "value": "\"}" }
        ],
        "chaos": null
      }
    },
    {
      "name": "catch-all",
      "conditions": { "all_of": [], "any_of": [] },
      "response": {
        "status": 200,
        "headers": [{ "name": "Content-Type", "value": "application/json" }],
        "body": [{ "type": "Literal", "value": "{\"greeting\":\"Hello stranger\"}" }],
        "chaos": null
      }
    }
  ]
}
'@
$r = Api "PUT" "/api/services/demo-api" $svc
Test "PUT service returns 200" "200" $r.Status
Test "Service name in response" "demo-api" $r.Body

# 3. Lister les services
Write-Host "`n[3] Lister les services"
$r = Api "GET" "/api/services"
Test "GET /api/services returns 200" "200" $r.Status
Test "List contains demo-api" "demo-api" $r.Body

# 4. Test mock interception - regle matchee
Write-Host "`n[4] Mock interception (regle matchee)"
$r = Api "GET" "/demo-api/anything?name=alice"
Test "Mock returns 200" "200" $r.Status
Test "Mock returns Alice greeting" "Hello Alice" $r.Body

# 5. Test mock interception - catch-all
Write-Host "`n[5] Mock interception (catch-all)"
$r = Api "GET" "/demo-api/anything?name=bob"
Test "Catch-all returns 200" "200" $r.Status
Test "Catch-all returns stranger" "Hello stranger" $r.Body

# 6. Toggle service OFF (proxy mode)
Write-Host "`n[6] Basculer en mode proxy"
$r = Api "PUT" "/api/services/demo-api/toggle" '{"is_mocked": false}'
Test "Toggle returns 200" "200" $r.Status
Test "is_mocked is false" "false" $r.Body

# 7. Test proxy forwarding (vers httpbin.org)
Write-Host "`n[7] Proxy forwarding"
$r = Api "GET" "/demo-api/get?test=proxy"
Test "Proxy returns 200" "200" $r.Status
Test "Proxy response from httpbin" "test.*proxy" $r.Body

# 8. Toggle back ON (mock mode)
Write-Host "`n[8] Rebasculer en mode mock"
$r = Api "PUT" "/api/services/demo-api/toggle" '{"is_mocked": true}'
Test "Toggle ON returns 200" "200" $r.Status
Test "is_mocked is true" "true" $r.Body

# 9. Verify mock works again after toggle
Write-Host "`n[9] Mock fonctionne apres retoggle"
$r = Api "GET" "/demo-api/anything?name=alice"
Test "Mock re-active returns Alice" "Hello Alice" $r.Body

# 10. Reorder rules
Write-Host "`n[10] Reordonner les regles"
$r = Api "PUT" "/api/services/demo-api/rules/reorder" '{"order": ["catch-all", "hello-rule"]}'
Test "Reorder returns 200" "200" $r.Status
# Now catch-all is first, so alice should get stranger
$r = Api "GET" "/demo-api/anything?name=alice"
Test "After reorder catch-all first" "Hello stranger" $r.Body

# 11. Serve frontend
Write-Host "`n[11] Frontend servi"
$r = Api "GET" "/"
Test "Index.html served" "200" $r.Status
Test "HTML contains lightMock" "lightMock" $r.Body

# 12. Delete service
Write-Host "`n[12] Supprimer le service"
$r = Api "DELETE" "/api/services/demo-api"
Test "Delete returns 204" "204" $r.Status

$r = Api "GET" "/api/services"
Test "Services list empty after delete" "\[\]" $r.Body

# 13. YAML persisted on disk
Write-Host "`n[13] Persistance YAML"
$yaml = Get-Content "data\mock-config.yaml" -Raw
Test "YAML file exists and valid" "services" $yaml

Write-Host "`n=== RESULTATS ===" -ForegroundColor Cyan
Write-Host "  $pass PASS / $($pass + $fail) total" -ForegroundColor $(if ($fail -eq 0) { "Green" } else { "Red" })
if ($fail -gt 0) { Write-Host "  $fail FAIL" -ForegroundColor Red }
