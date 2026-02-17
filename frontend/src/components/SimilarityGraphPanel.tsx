import { useCallback, useEffect, useMemo, useRef, useState, type PointerEvent as ReactPointerEvent, type WheelEvent as ReactWheelEvent } from 'react';
import { ExternalLink, Move, RefreshCw, ZoomIn, ZoomOut } from 'lucide-react';
import { Card } from './ui/card';
import { Button } from './ui/button';
import { Input } from './ui/input';
import { useTheme } from '../contexts/ThemeContext';
import * as api from '../api/client';
import type { SimilarityEdge, SimilarityGraphNode, SimilarityGraphResponse } from '../api/types';

type SimNode = SimilarityGraphNode & {
  x: number;
  y: number;
  vx: number;
  vy: number;
};

type Viewport = {
  scale: number;
  x: number;
  y: number;
};

const MIN_SCALE = 0.4;
const MAX_SCALE = 3;
const EDGE_LENGTH = 120;
const REPULSION = 18000;
const SPRING_K = 0.0018;

type DocType = 'audio' | 'document' | 'other';

const AUDIO_EXTENSIONS = new Set(['.mp3', '.wav', '.m4a', '.aac', '.ogg', '.flac', '.wma', '.opus']);
const DOCUMENT_EXTENSIONS = new Set(['.txt', '.md', '.pdf', '.doc', '.docx', '.ppt', '.pptx', '.xls', '.xlsx', '.hwp']);

const resolveDocType = (filePath?: string): DocType => {
  if (!filePath) return 'other';
  const idx = filePath.lastIndexOf('.');
  const ext = idx >= 0 ? filePath.slice(idx).toLowerCase() : '';
  if (AUDIO_EXTENSIONS.has(ext)) return 'audio';
  if (DOCUMENT_EXTENSIONS.has(ext)) return 'document';
  return 'other';
};

export function SimilarityGraphPanel() {
  const { theme } = useTheme();
  const svgRef = useRef<SVGSVGElement | null>(null);
  const [size, setSize] = useState({ width: 980, height: 620 });

  const [minSimilarity, setMinSimilarity] = useState(0.65);
  const [maxNodes, setMaxNodes] = useState(120);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [graph, setGraph] = useState<SimilarityGraphResponse | null>(null);
  const [nodes, setNodes] = useState<SimNode[]>([]);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);

  const [viewport, setViewport] = useState<Viewport>({ scale: 1, x: 0, y: 0 });
  const dragStateRef = useRef<{ mode: 'none' | 'pan' | 'node'; id?: string; sx: number; sy: number }>(
    { mode: 'none', sx: 0, sy: 0 },
  );

  const selectedNode = useMemo(
    () => nodes.find((node) => node.id === selectedNodeId) ?? null,
    [nodes, selectedNodeId],
  );

  const nodeMap = useMemo(() => {
    return new Map(nodes.map((node) => [node.id, node]));
  }, [nodes]);

  const edgeStats = useMemo(() => {
    const weights = graph?.edges?.map((edge) => edge.weight) ?? [];
    const min = weights.length ? Math.min(...weights) : minSimilarity;
    const max = weights.length ? Math.max(...weights) : minSimilarity;
    return { min, max };
  }, [graph?.edges, minSimilarity]);

  const degreeMap = useMemo(() => {
    const map = new Map<string, number>();
    for (const node of graph?.nodes ?? []) {
      map.set(node.id, 0);
    }
    for (const edge of graph?.edges ?? []) {
      map.set(edge.source, (map.get(edge.source) ?? 0) + 1);
      map.set(edge.target, (map.get(edge.target) ?? 0) + 1);
    }
    return map;
  }, [graph?.nodes, graph?.edges]);

  const maxDegree = useMemo(() => {
    if (degreeMap.size === 0) return 1;
    return Math.max(...degreeMap.values(), 1);
  }, [degreeMap]);

  const worldFromClient = useCallback((clientX: number, clientY: number) => {
    const rect = svgRef.current?.getBoundingClientRect();
    if (!rect) return { x: 0, y: 0 };
    const localX = clientX - rect.left;
    const localY = clientY - rect.top;
    return {
      x: (localX - viewport.x) / viewport.scale,
      y: (localY - viewport.y) / viewport.scale,
    };
  }, [viewport]);

  const initPositions = useCallback((data: SimilarityGraphResponse) => {
    const cx = size.width / 2;
    const cy = size.height / 2;
    const radius = Math.max(80, Math.min(size.width, size.height) * 0.28);
    const seededNodes = data.nodes.map((node, idx) => {
      const angle = (Math.PI * 2 * idx) / Math.max(1, data.nodes.length);
      const jitter = ((idx % 7) - 3) * 7;
      return {
        ...node,
        x: cx + Math.cos(angle) * (radius + jitter),
        y: cy + Math.sin(angle) * (radius + jitter),
        vx: 0,
        vy: 0,
      };
    });
    setNodes(seededNodes);
    setSelectedNodeId(seededNodes[0]?.id ?? null);
    setViewport({ scale: 1, x: 0, y: 0 });
  }, [size.height, size.width]);

  const loadGraph = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const payload = await api.getSimilarityGraph({
        min_similarity: minSimilarity,
        max_nodes: maxNodes,
        sampling: 'hybrid',
      });
      setGraph(payload);
      initPositions(payload);
    } catch (e) {
      setError(e instanceof Error ? e.message : '그래프를 불러오지 못했습니다.');
      setGraph(null);
      setNodes([]);
      setSelectedNodeId(null);
    } finally {
      setLoading(false);
    }
  }, [initPositions, maxNodes, minSimilarity]);

  useEffect(() => {
    loadGraph();
  }, [loadGraph]);

  useEffect(() => {
    const svg = svgRef.current;
    if (!svg || typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      setSize({ width: entry.contentRect.width, height: entry.contentRect.height });
    });
    observer.observe(svg);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (!graph || nodes.length === 0) return;
    let rafId = 0;
    const edgeList = graph.edges;

    const tick = () => {
      setNodes((prev) => {
        if (prev.length <= 1) return prev;
        const next = prev.map((node) => ({ ...node }));

        for (let i = 0; i < next.length; i += 1) {
          for (let j = i + 1; j < next.length; j += 1) {
            const a = next[i];
            const b = next[j];
            const dx = b.x - a.x;
            const dy = b.y - a.y;
            const dist2 = Math.max(25, dx * dx + dy * dy);
            const force = REPULSION / dist2;
            const fx = (force * dx) / Math.sqrt(dist2);
            const fy = (force * dy) / Math.sqrt(dist2);
            a.vx -= fx;
            a.vy -= fy;
            b.vx += fx;
            b.vy += fy;
          }
        }

        for (const edge of edgeList) {
          const source = next.find((node) => node.id === edge.source);
          const target = next.find((node) => node.id === edge.target);
          if (!source || !target) continue;
          const dx = target.x - source.x;
          const dy = target.y - source.y;
          const dist = Math.max(1, Math.sqrt(dx * dx + dy * dy));
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
          if (dragStateRef.current.mode === 'node' && dragStateRef.current.id === node.id) {
            node.vx = 0;
            node.vy = 0;
            continue;
          }
          node.vx *= 0.86;
          node.vy *= 0.86;
          node.x += node.vx;
          node.y += node.vy;
          node.x = Math.max(-200, Math.min(size.width + 200, node.x));
          node.y = Math.max(-200, Math.min(size.height + 200, node.y));
        }

        return next;
      });

      rafId = window.requestAnimationFrame(tick);
    };

    rafId = window.requestAnimationFrame(tick);
    return () => window.cancelAnimationFrame(rafId);
  }, [graph, nodes.length, size.height, size.width]);

  const edgeWidth = (edge: SimilarityEdge) => {
    const range = Math.max(0.0001, edgeStats.max - edgeStats.min);
    const normalized = Math.max(0, Math.min(1, (edge.weight - edgeStats.min) / range));
    return 1.2 + normalized * 3.2;
  };

  const edgeOpacity = (edge: SimilarityEdge) => {
    const range = Math.max(0.0001, edgeStats.max - edgeStats.min);
    const normalized = Math.max(0, Math.min(1, (edge.weight - edgeStats.min) / range));
    return 0.2 + normalized * 0.7;
  };

  const getNodeStyle = (node: SimNode) => {
    const docType = resolveDocType(node.file);
    const degree = degreeMap.get(node.id) ?? 0;
    const centrality = degree / Math.max(1, maxDegree);
    const radius = 7 + centrality * 6;

    if (docType === 'audio') return { radius, fill: '#38bdf8', stroke: '#bae6fd' };
    if (docType === 'document') return { radius, fill: '#34d399', stroke: '#a7f3d0' };
    return { radius, fill: '#a78bfa', stroke: '#ddd6fe' };
  };

  const handlePointerMove = (event: ReactPointerEvent<SVGSVGElement>) => {
    const drag = dragStateRef.current;
    if (drag.mode === 'none') return;

    if (drag.mode === 'pan') {
      setViewport((prev) => ({ ...prev, x: prev.x + (event.clientX - drag.sx), y: prev.y + (event.clientY - drag.sy) }));
      dragStateRef.current = { ...drag, sx: event.clientX, sy: event.clientY };
      return;
    }

    if (drag.mode === 'node' && drag.id) {
      const world = worldFromClient(event.clientX, event.clientY);
      setNodes((prev) => prev.map((node) => (node.id === drag.id ? { ...node, x: world.x, y: world.y, vx: 0, vy: 0 } : node)));
    }
  };

  const handlePointerUp = () => {
    dragStateRef.current = { mode: 'none', sx: 0, sy: 0 };
  };

  const startPan = (event: ReactPointerEvent<SVGSVGElement>) => {
    if (event.target !== svgRef.current) return;
    dragStateRef.current = { mode: 'pan', sx: event.clientX, sy: event.clientY };
  };

  const startNodeDrag = (event: ReactPointerEvent<SVGCircleElement>, nodeId: string) => {
    event.stopPropagation();
    dragStateRef.current = { mode: 'node', id: nodeId, sx: event.clientX, sy: event.clientY };
  };

  const handleWheel = (event: ReactWheelEvent<SVGSVGElement>) => {
    event.preventDefault();
    const delta = -event.deltaY * 0.0012;
    const nextScale = Math.max(MIN_SCALE, Math.min(MAX_SCALE, viewport.scale * (1 + delta)));
    if (nextScale === viewport.scale) return;

    const rect = svgRef.current?.getBoundingClientRect();
    if (!rect) return;
    const cursorX = event.clientX - rect.left;
    const cursorY = event.clientY - rect.top;
    const worldX = (cursorX - viewport.x) / viewport.scale;
    const worldY = (cursorY - viewport.y) / viewport.scale;

    setViewport({
      scale: nextScale,
      x: cursorX - worldX * nextScale,
      y: cursorY - worldY * nextScale,
    });
  };

  const zoomBy = (factor: number) => {
    setViewport((prev) => {
      const nextScale = Math.max(MIN_SCALE, Math.min(MAX_SCALE, prev.scale * factor));
      return { ...prev, scale: nextScale };
    });
  };

  const resetView = () => setViewport({ scale: 1, x: 0, y: 0 });

  return (
    <Card className={`backdrop-blur-sm ${theme === 'dark' ? 'border-slate-800 bg-slate-900/50' : 'border-slate-200 bg-white/70'}`}>
      <div className="p-6 space-y-4">
        <div className="flex flex-wrap items-end gap-3">
          <div className="space-y-1">
            <label className="text-xs font-medium">min similarity</label>
            <Input type="number" min={0.3} max={0.99} step={0.05} value={minSimilarity}
              onChange={(e) => setMinSimilarity(Number(e.target.value || 0.65))} className="w-32" />
          </div>
          <div className="space-y-1">
            <label className="text-xs font-medium">max nodes</label>
            <Input type="number" min={20} max={180} step={10} value={maxNodes}
              onChange={(e) => setMaxNodes(Number(e.target.value || 120))} className="w-28" />
          </div>
          <Button onClick={() => void loadGraph()} disabled={loading} className="gap-2">
            <RefreshCw className={`size-4 ${loading ? 'animate-spin' : ''}`} /> Reload
          </Button>
          <Button type="button" variant="outline" onClick={() => zoomBy(1.15)} className="gap-2"><ZoomIn className="size-4" /> 줌 인</Button>
          <Button type="button" variant="outline" onClick={() => zoomBy(1 / 1.15)} className="gap-2"><ZoomOut className="size-4" /> 줌 아웃</Button>
          <Button type="button" variant="outline" onClick={resetView} className="gap-2"><Move className="size-4" /> 뷰 리셋</Button>
          <div className={`text-xs ml-auto ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>
            {graph ? `nodes ${graph.nodes.length} / edges ${graph.edges.length}` : '그래프 없음'}
          </div>
        </div>

        <div className={`grid gap-2 text-xs ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'} lg:grid-cols-[1fr_auto]`}>
          <div>마우스 휠 줌, 배경 드래그 팬, 노드 드래그 이동, 노드 클릭 선택.</div>
          <div className="flex items-center gap-4">
            <div className="flex items-center gap-2">
              <span className="font-medium">노드 타입</span>
              <span className="inline-flex items-center gap-1"><span className="size-2 rounded-full bg-sky-400" />audio</span>
              <span className="inline-flex items-center gap-1"><span className="size-2 rounded-full bg-emerald-400" />document</span>
              <span className="inline-flex items-center gap-1"><span className="size-2 rounded-full bg-violet-400" />other</span>
            </div>
            <div className="flex items-center gap-2">
              <span className="font-medium">edge weight</span>
              <svg width="90" height="18" viewBox="0 0 90 18" aria-label="edge weight legend">
                <line x1="4" y1="9" x2="28" y2="9" stroke={theme === 'dark' ? '#64748b' : '#94a3b8'} strokeWidth={1.4} strokeOpacity={0.25} />
                <line x1="33" y1="9" x2="57" y2="9" stroke={theme === 'dark' ? '#64748b' : '#94a3b8'} strokeWidth={2.8} strokeOpacity={0.55} />
                <line x1="62" y1="9" x2="86" y2="9" stroke={theme === 'dark' ? '#64748b' : '#94a3b8'} strokeWidth={4.2} strokeOpacity={0.9} />
              </svg>
              <span>{edgeStats.min.toFixed(2)}~{edgeStats.max.toFixed(2)}</span>
            </div>
          </div>
        </div>

        {error && <div className="text-sm text-red-500">{error}</div>}

        <div className={`grid gap-4 ${selectedNode ? 'lg:grid-cols-[1fr_260px]' : 'grid-cols-1'}`}>
          <svg
            ref={svgRef}
            className={`h-[640px] w-full rounded-xl border ${theme === 'dark' ? 'border-slate-800 bg-slate-950/70' : 'border-slate-200 bg-slate-50'}`}
            onWheel={handleWheel}
            onPointerMove={handlePointerMove}
            onPointerUp={handlePointerUp}
            onPointerLeave={handlePointerUp}
            onPointerDown={startPan}
          >
            <g transform={`translate(${viewport.x}, ${viewport.y}) scale(${viewport.scale})`}>
              {(graph?.edges ?? []).map((edge) => {
                const source = nodeMap.get(edge.source);
                const target = nodeMap.get(edge.target);
                if (!source || !target) return null;
                return (
                  <line
                    key={`${edge.source}-${edge.target}`}
                    x1={source.x}
                    y1={source.y}
                    x2={target.x}
                    y2={target.y}
                    stroke={theme === 'dark' ? '#334155' : '#94a3b8'}
                    strokeOpacity={edgeOpacity(edge)}
                    strokeWidth={edgeWidth(edge)}
                  />
                );
              })}
              {nodes.map((node) => {
                const selected = node.id === selectedNodeId;
                const style = getNodeStyle(node);
                return (
                  <g key={node.id}>
                    <circle
                      cx={node.x}
                      cy={node.y}
                      r={selected ? style.radius + 2 : style.radius}
                      fill={selected ? '#f59e0b' : style.fill}
                      stroke={selected ? '#fde68a' : style.stroke}
                      strokeWidth={selected ? 2 : 1}
                      onPointerDown={(event) => startNodeDrag(event, node.id)}
                      onClick={(event) => {
                        event.stopPropagation();
                        setSelectedNodeId(node.id);
                      }}
                    />
                    <text
                      x={node.x + 12}
                      y={node.y - 10}
                      fontSize={11}
                      fill={theme === 'dark' ? '#cbd5e1' : '#334155'}
                    >
                      {(node.label || node.id).slice(0, 22)}
                    </text>
                  </g>
                );
              })}
            </g>
          </svg>

          {selectedNode && (
            <aside className={`rounded-xl border p-4 text-sm ${theme === 'dark' ? 'border-slate-800 bg-slate-900/70' : 'border-slate-200 bg-white'}`}>
              <h3 className="font-semibold mb-2">선택 문서</h3>
              <div className="space-y-2 break-all">
                <div><span className="font-medium">이름:</span> {selectedNode.label || selectedNode.id}</div>
                <div><span className="font-medium">id:</span> {selectedNode.id}</div>
                <div><span className="font-medium">record:</span> {selectedNode.record_id || '-'}</div>
                <div><span className="font-medium">업로드:</span> {selectedNode.uploaded_at || '-'}</div>
              </div>
              <Button
                className="mt-4 w-full gap-2"
                onClick={() => window.open(api.getDownloadUrl(selectedNode.id), '_blank', 'noopener,noreferrer')}
              >
                <ExternalLink className="size-4" /> 다운로드 열기
              </Button>
            </aside>
          )}
        </div>
      </div>
    </Card>
  );
}
