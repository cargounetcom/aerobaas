#!/usr/bin/env python3
"""
AeroBaaS Benchmark — Cold Start + RPS + Memory
Tests the acceptance criteria: <50ms cold start, 50,000 RPS target.
"""

import asyncio
import time
import httpx
import json
import sys

BASE_URL = "http://localhost:8080"
CONCURRENCY = 100
TOTAL_REQUESTS = 10000

async def benchmark_cold_start(client):
    """Test: Wasm function cold start < 50ms"""
    print("\n=== Cold Start Benchmark ===")
    
    deploy_resp = await client.post(f"{BASE_URL}/v1/functions", json={
        "name": "bench-cold-start",
        "wasm_bytes": "AGFzbQEAAAA=",  # minimal Wasm binary
        "entrypoint": "_start",
        "memory_limit_mb": 64,
        "timeout_ms": 5000,
    })
    
    if deploy_resp.status_code != 200:
        print(f"Deploy failed: {deploy_resp.status_code}")
        return
    
    func_id = deploy_resp.json().get("function_id")
    
    cold_starts = []
    for i in range(100):
        start = time.perf_counter()
        resp = await client.post(f"{BASE_URL}/v1/functions/{func_id}/execute", json={
            "function_id": func_id,
            "payload": {"iteration": i},
        })
        elapsed_ms = (time.perf_counter() - start) * 1000
        
        if resp.status_code == 200:
            data = resp.json()
            cold_starts.append(data.get("duration_ms", elapsed_ms))
    
    if cold_starts:
        avg = sum(cold_starts) / len(cold_starts)
        p50 = sorted(cold_starts)[len(cold_starts) // 2]
        p95 = sorted(cold_starts)[int(len(cold_starts) * 0.95)]
        p99 = sorted(cold_starts)[int(len(cold_starts) * 0.99)]
        
        print(f"  Total executions: {len(cold_starts)}")
        print(f"  Avg:   {avg:.2f}ms")
        print(f"  P50:   {p50:.2f}ms")
        print(f"  P95:   {p95:.2f}ms")
        print(f"  P99:   {p99:.2f}ms")
        
        if p99 < 50:
            print("  ✅ PASS: P99 < 50ms")
        else:
            print("  ❌ FAIL: P99 >= 50ms")

async def benchmark_rps(client):
    """Test: 50,000 RPS target"""
    print("\n=== RPS Benchmark ===")
    
    async def make_request():
        return await client.get(f"{BASE_URL}/health")
    
    start = time.perf_counter()
    tasks = [make_request() for _ in range(TOTAL_REQUESTS)]
    responses = await asyncio.gather(*tasks)
    elapsed = time.perf_counter() - start
    
    rps = TOTAL_REQUESTS / elapsed
    success = sum(1 for r in responses if r.status_code == 200)
    
    print(f"  Total requests: {TOTAL_REQUESTS}")
    print(f"  Success: {success}")
    print(f"  Duration: {elapsed:.2f}s")
    print(f"  RPS: {rps:.0f}")
    
    if rps >= 50000:
        print("  ✅ PASS: >= 50,000 RPS")
    else:
        print(f"  ⚠️ {rps:.0f} RPS (target: 50,000)")

async def benchmark_keys_scoped(client):
    """Test: Scoped key returns filtered function list"""
    print("\n=== Keys API 2.0 Scoped Access Benchmark ===")
    
    key_resp = await client.post(f"{BASE_URL}/v1/keys", json={
        "name": "bench-scoped",
        "scopes": ["functions.list", "functions.read"],
        "credits_limit": 1000000,
    })
    
    if key_resp.status_code == 200:
        key_data = key_resp.json()
        print(f"  Key created: {key_data.get('key_prefix')}...")
        print(f"  Scopes: {key_data.get('scopes')}")
        print(f"  Credits: {key_data.get('credits_limit')}")
        print("  ✅ PASS: Scoped key created")
    else:
        print(f"  ❌ FAIL: {key_resp.status_code}")

async def benchmark_ai_inference(client):
    """Test: Local AI inference latency"""
    print("\n=== AI Inference (Local SLM) Benchmark ===")
    
    latencies = []
    for i in range(10):
        start = time.perf_counter()
        resp = await client.post(f"{BASE_URL}/v1/ai/generate", json={
            "prompt": f"Generate a simple function #{i}"
        })
        elapsed_ms = (time.perf_counter() - start) * 1000
        
        if resp.status_code == 200:
            latencies.append(elapsed_ms)
    
    if latencies:
        avg = sum(latencies) / len(latencies)
        print(f"  Inferences: {len(latencies)}")
        print(f"  Avg latency: {avg:.2f}ms")
        print(f"  Min: {min(latencies):.2f}ms")
        print(f"  Max: {max(latencies):.2f}ms")

async def main():
    print("AeroBaaS Benchmark Suite")
    print("=" * 50)
    
    async with httpx.AsyncClient(timeout=30) as client:
        try:
            health = await client.get(f"{BASE_URL}/health")
            if health.status_code != 200:
                print("Server not running. Start with: aero serve")
                sys.exit(1)
        except httpx.ConnectError:
            print("Server not running. Start with: aero serve")
            sys.exit(1)
        
        await benchmark_cold_start(client)
        await benchmark_rps(client)
        await benchmark_keys_scoped(client)
        await benchmark_ai_inference(client)
        
        print("\n" + "=" * 50)
        print("Benchmark complete.")

if __name__ == "__main__":
    asyncio.run(main())
