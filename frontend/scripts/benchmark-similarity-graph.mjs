#!/usr/bin/env node
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { performance } from 'node:perf_hooks';

const DATASET_SIZES = [100, 500, 1000];
const ITERATIONS = 10;
const EDGE_LENGTH = 120;
const REPULSION = 18000;
const SPRING_K = 0.0018;

function parseArgs(argv) {
  const options = {
    outDir: '../docs/perf',
    apiBaseUrl: null,
    minSimilarity: 0.65,
    sampling: 'hybrid',
    candidateStrategy: 'auto',
    mockResponseDelayMs: 8,
  };

  for (const arg of argv) {
    if (arg.startsWith('--out-dir=')) {
      options.outDir = arg.slice('--out-dir='.length);
    } else if (arg.startsWith('--api-base-url=')) {
      options.apiBaseUrl = arg.slice('--api-base-url='.length);
    } else if (arg.startsWith('--min-similarity=')) {
      const parsed = Number(arg.slice('--min-similarity='.length));
      if (!Number.isNaN(parsed)) options.minSimilarity = parsed;
    } else if (arg.startsWith('--sampling=')) {
      options.sampling = arg.slice('--sampling='.length) || 'hybrid';
    } else if (arg.startsWith('--candidate-strategy=')) {
      options.candidateStrategy = arg.slice('--candidate-strategy='.length) || 'auto';
    } else if (arg.startsWith('--mock-response-delay-ms=')) {
      const parsed = Number(arg.slice('--mock-response-delay-ms='.length));
      if (!Number.isNaN(parsed) && parsed >= 0) options.mockResponseDelayMs = parsed;
    }
  }

  return options;
}

function createSyntheticGraph(nodeCount) {
  const nodes = Array.from({ length: nodeCount }, (_, index) => ({
    id: `doc-${index + 1}`,
    label: `Document ${index + 1}`,
    file: `DB/uploads/doc-${index + 1}.md`,
    record_id: `record-${index + 1}`,
    uploaded_at: '2026-01-01T00:00:00Z',
  }));

  const edges = [];
  for (let i = 0; i < nodeCount; i += 1) {
    for (let step = 1; step <= 3; step += 1) {
      const target = i + step;
      if (target < nodeCount) {
        edges.push({
          source: `doc-${i + 1}`,
          target: `doc-${target + 1}`,
          weight: Number((0.65 + (step * 0.08)).toFixed(3)),
        });
      }
    }
  }

  return { nodes, edges };
}

function initPositions(graph, width = 980, height = 620) {
  const cx = width / 2;
  const cy = height / 2;
  const radius = Math.max(80, Math.min(width, height) * 0.28);

  return graph.nodes.map((node, idx) => {
    const angle = (Math.PI * 2 * idx) / Math.max(1, graph.nodes.length);
    const jitter = ((idx % 7) - 3) * 7;
    return {
      ...node,
      x: cx + (Math.cos(angle) * (radius + jitter)),
      y: cy + (Math.sin(angle) * (radius + jitter)),
      vx: 0,
      vy: 0,
    };
  });
}

function runSinglePhysicsTick(nodes, edges) {
  const next = nodes.map((node) => ({ ...node }));

  for (let i = 0; i < next.length; i += 1) {
    for (let j = i + 1; j < next.length; j += 1) {
      const a = next[i];
      const b = next[j];
      const dx = b.x - a.x;
      const dy = b.y - a.y;
      const dist2 = Math.max(25, (dx * dx) + (dy * dy));
      const force = REPULSION / dist2;
      const fx = (force * dx) / Math.sqrt(dist2);
      const fy = (force * dy) / Math.sqrt(dist2);
      a.vx -= fx;
      a.vy -= fy;
      b.vx += fx;
      b.vy += fy;
    }
  }

  for (const edge of edges) {
    const source = next.find((node) => node.id === edge.source);
    const target = next.find((node) => node.id === edge.target);
    if (!source || !target) continue;
    const dx = target.x - source.x;
    const dy = target.y - source.y;
    const dist = Math.max(1, Math.sqrt((dx * dx) + (dy * dy)));
    const displacement = dist - EDGE_LENGTH;
    const force = SPRING_K * displacement;
    const fx = (force * dx) / dist;
    const fy = (force * dy) / dist;
    source.vx += fx;
    source.vy += fy;
    target.vx -= fx;
    target.vy -= fy;
  }

  for (const node of next) {
    node.vx *= 0.86;
    node.vy *= 0.86;
    node.x += node.vx;
    node.y += node.vy;
  }

  return next;
}

function estimateSvgRender(nodes, edges) {
  const nodeMap = new Map(nodes.map((node) => [node.id, node]));

  let lineCount = 0;
  for (const edge of edges) {
    const source = nodeMap.get(edge.source);
    const target = nodeMap.get(edge.target);
    if (!source || !target) continue;
    lineCount += 1;
  }

  let labelBytes = 0;
  for (const node of nodes) {
    labelBytes += (node.label || node.id).slice(0, 22).length;
  }

  return { lineCount, circleCount: nodes.length, labelBytes };
}

async function measureResponseTimeMs(size, options) {
  if (!options.apiBaseUrl) {
    if (options.mockResponseDelayMs > 0) {
      await new Promise((resolveDelay) => setTimeout(resolveDelay, options.mockResponseDelayMs));
    }
    return options.mockResponseDelayMs;
  }

  const url = new URL('/api/similarity-graph', options.apiBaseUrl);
  url.searchParams.set('min_similarity', String(options.minSimilarity));
  url.searchParams.set('max_nodes', String(size));
  url.searchParams.set('sampling', options.sampling);
  url.searchParams.set('candidate_strategy', options.candidateStrategy);

  const start = performance.now();
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Graph API failed for size=${size}: ${response.status} ${response.statusText}`);
  }
  await response.json();
  return performance.now() - start;
}

function mean(values) {
  return Number((values.reduce((sum, value) => sum + value, 0) / values.length).toFixed(2));
}

function percentile(values, targetPercentile) {
  const sorted = [...values].sort((a, b) => a - b);
  const position = Math.max(0, Math.ceil((targetPercentile / 100) * sorted.length) - 1);
  return Number(sorted[Math.min(position, sorted.length - 1)].toFixed(2));
}

async function benchmarkSize(size, options) {
  const syntheticGraph = createSyntheticGraph(size);
  const samples = [];

  for (let i = 0; i < ITERATIONS; i += 1) {
    const responseTimeMs = await measureResponseTimeMs(size, options);

    const renderStart = performance.now();
    const seededNodes = initPositions(syntheticGraph);
    const settledNodes = runSinglePhysicsTick(seededNodes, syntheticGraph.edges);
    const renderMeta = estimateSvgRender(settledNodes, syntheticGraph.edges);
    const renderTimeMs = performance.now() - renderStart;

    samples.push({
      iteration: i + 1,
      responseTimeMs: Number(responseTimeMs.toFixed(2)),
      renderTimeMs: Number(renderTimeMs.toFixed(2)),
      circles: renderMeta.circleCount,
      lines: renderMeta.lineCount,
    });
  }

  const responseSeries = samples.map((sample) => sample.responseTimeMs);
  const renderSeries = samples.map((sample) => sample.renderTimeMs);

  return {
    nodeCount: size,
    edges: syntheticGraph.edges.length,
    samples,
    summary: {
      responseAvgMs: mean(responseSeries),
      responseP95Ms: percentile(responseSeries, 95),
      renderAvgMs: mean(renderSeries),
      renderP95Ms: percentile(renderSeries, 95),
    },
  };
}

function makeMarkdownReport(result) {
  const rows = result.scenarios
    .map((scenario) => `| ${scenario.nodeCount} | ${scenario.edges} | ${scenario.summary.responseAvgMs} / ${scenario.summary.responseP95Ms} | ${scenario.summary.renderAvgMs} / ${scenario.summary.renderP95Ms} |`)
    .join('\n');

  return `# Similarity Graph Benchmark Baseline

- Generated at: ${result.generatedAt}
- Response source: ${result.mode === 'real_api' ? `real API (${result.apiBaseUrl})` : `mock delay (${result.mockResponseDelayMs} ms)`}
- Iterations per scenario: ${result.iterations}
- Dataset sizes: ${result.scenarios.map((scenario) => scenario.nodeCount).join(', ')}
- Sampling strategy: ${result.sampling}
- Candidate strategy: ${result.candidateStrategy}

## Summary

| Documents | Edges | Response Avg/P95 (ms) | Render Avg/P95 (ms) |
| --- | --- | --- | --- |
${rows}

## Measurement definition

- **Response**: \`/api/similarity-graph\` request/response round trip time.
- **Render**: SimilarityGraphPanel의 핵심 연산(초기 좌표 시드 + force tick 1회 + SVG element 구성 루프)을 Node 런타임에서 실행한 시간.
- 현재 렌더 측정은 브라우저 페인트 시간 전체가 아니라, 컴포넌트의 자료구조/루프 비용에 대한 CPU 기준선입니다.
`;
}

async function run() {
  const options = parseArgs(process.argv.slice(2));
  const scenarios = [];

  for (const size of DATASET_SIZES) {
    // eslint-disable-next-line no-await-in-loop
    scenarios.push(await benchmarkSize(size, options));
  }

  const result = {
    generatedAt: new Date().toISOString(),
    mode: options.apiBaseUrl ? 'real_api' : 'mock_api',
    apiBaseUrl: options.apiBaseUrl,
    mockResponseDelayMs: options.mockResponseDelayMs,
    iterations: ITERATIONS,
    sampling: options.sampling,
    candidateStrategy: options.candidateStrategy,
    scenarios,
  };

  const outDir = resolve(options.outDir);
  mkdirSync(outDir, { recursive: true });

  const jsonPath = resolve(outDir, 'similarity-graph-baseline.json');
  writeFileSync(jsonPath, `${JSON.stringify(result, null, 2)}\n`, 'utf8');

  const mdPath = resolve(outDir, 'similarity-graph-baseline.md');
  writeFileSync(mdPath, makeMarkdownReport(result), 'utf8');

  console.log(`Generated benchmark baseline:\n- ${jsonPath}\n- ${mdPath}`);
}

run().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
