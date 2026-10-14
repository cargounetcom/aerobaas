import { useMetrics } from "@/hooks/use-metrics";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Activity,
  Cpu,
  MemoryStick,
  Gauge,
  Zap,
  ShieldAlert,
  Clock,
  TrendingUp,
} from "lucide-react";
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  ReferenceLine,
  Area,
  AreaChart,
} from "recharts";

const statusColors: Record<string, string> = {
  idle: "bg-gray-500/10 text-gray-600 border-gray-500/20",
  running: "bg-green-500/10 text-green-600 border-green-500/20",
  cold_start: "bg-blue-500/10 text-blue-600 border-blue-500/20",
  quarantine: "bg-yellow-500/10 text-yellow-600 border-yellow-500/20",
  blocked: "bg-red-500/10 text-red-600 border-red-500/20",
  crashed: "bg-red-500/10 text-red-600 border-red-500/20",
};

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function StatCard({
  title,
  value,
  unit,
  icon: Icon,
  subtitle,
}: {
  title: string;
  value: string | number;
  unit?: string;
  icon: typeof Activity;
  subtitle?: string;
}) {
  return (
    <Card>
      <CardContent className="p-4">
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm text-muted-foreground">{title}</p>
            <p className="text-2xl font-bold">
              {value}
              {unit && <span className="text-sm text-muted-foreground ml-1">{unit}</span>}
            </p>
            {subtitle && <p className="text-xs text-muted-foreground mt-1">{subtitle}</p>}
          </div>
          <div className="h-10 w-10 rounded-lg bg-primary/10 flex items-center justify-center">
            <Icon className="h-5 w-5 text-primary" />
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

export function MetricsDashboard() {
  const { snapshot, history, loading } = useMetrics(5000);

  if (loading || !snapshot) {
    return (
      <div className="container mx-auto py-6">
        <div className="flex items-center justify-center py-20 text-muted-foreground">
          Завантаження метрик...
        </div>
      </div>
    );
  }

  const agg = snapshot.aggregate;

  const coldStartData = (history?.cold_start_history || []).map((p) => ({
    time: new Date(p.timestamp).toLocaleTimeString("uk-UA", { hour: "2-digit", minute: "2-digit", second: "2-digit" }),
    cold_start_ms: p.cold_start_ms,
    function: p.function_name,
  }));

  const memoryData = (history?.memory_history || []).map((p) => ({
    time: new Date(p.timestamp).toLocaleTimeString("uk-UA", { hour: "2-digit", minute: "2-digit", second: "2-digit" }),
    used_mb: p.memory_used_mb,
    limit_mb: p.memory_limit_mb,
    throttled: p.throttled,
    function: p.function_name,
  }));

  return (
    <div className="container mx-auto py-6 space-y-6">
      <div>
        <h1 className="text-2xl font-bold">Wasm Sandbox Metrics</h1>
        <p className="text-muted-foreground">Продуктивність безсерверних контейнерів у реальному часі</p>
      </div>

      {/* Stat Cards */}
      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        <StatCard
          title="Cold Start (avg)"
          value={agg.avg_cold_start_ms.toFixed(1)}
          unit="ms"
          icon={Zap}
          subtitle={`P50: ${agg.p50_cold_start_ms.toFixed(1)}ms · P99: ${agg.p99_cold_start_ms.toFixed(1)}ms`}
        />
        <StatCard
          title="Active Sandboxes"
          value={agg.active_sandboxes}
          icon={Activity}
          subtitle={`Idle: ${agg.idle_sandboxes} · Blocked: ${agg.blocked_sandboxes}`}
        />
        <StatCard
          title="Memory (avg)"
          value={agg.avg_memory_mb.toFixed(1)}
          unit="MB"
          icon={MemoryStick}
          subtitle={`Peak: ${agg.max_memory_mb.toFixed(1)}MB`}
        />
        <StatCard
          title="Requests/sec"
          value={agg.rps.toFixed(0)}
          icon={Gauge}
          subtitle={`Total: ${agg.total_invocations} · Errors: ${agg.total_errors}`}
        />
      </div>

      {/* Charts */}
      <div className="grid gap-4 md:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Clock className="h-5 w-5" />
              Cold Start Latency (ms)
            </CardTitle>
          </CardHeader>
          <CardContent>
            <ResponsiveContainer width="100%" height={250}>
              <LineChart data={coldStartData}>
                <CartesianGrid strokeDasharray="3 3" className="stroke-muted" />
                <XAxis dataKey="time" className="text-xs" />
                <YAxis className="text-xs" unit="ms" />
                <Tooltip
                  contentStyle={{ background: "hsl(var(--background))", border: "1px solid hsl(var(--border))" }}
                />
                <ReferenceLine y={50} stroke="#22c55e" strokeDasharray="5 5" label="50ms target" />
                <Line
                  type="monotone"
                  dataKey="cold_start_ms"
                  stroke="hsl(var(--primary))"
                  strokeWidth={2}
                  dot={false}
                />
              </LineChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <MemoryStick className="h-5 w-5" />
              Memory Throttling (MB)
            </CardTitle>
          </CardHeader>
          <CardContent>
            <ResponsiveContainer width="100%" height={250}>
              <AreaChart data={memoryData}>
                <CartesianGrid strokeDasharray="3 3" className="stroke-muted" />
                <XAxis dataKey="time" className="text-xs" />
                <YAxis className="text-xs" unit="MB" />
                <Tooltip
                  contentStyle={{ background: "hsl(var(--background))", border: "1px solid hsl(var(--border))" }}
                />
                <Area
                  type="monotone"
                  dataKey="used_mb"
                  stroke="hsl(var(--primary))"
                  fill="hsl(var(--primary))"
                  fillOpacity={0.2}
                  strokeWidth={2}
                />
                <Line
                  type="monotone"
                  dataKey="limit_mb"
                  stroke="#ef4444"
                  strokeDasharray="5 5"
                  dot={false}
                />
              </AreaChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>
      </div>

      {/* Sandbox Table */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Cpu className="h-5 w-5" />
            Active Sandboxes ({snapshot.sandboxes.length})
          </CardTitle>
        </CardHeader>
        <CardContent>
          <ScrollArea className="h-[400px]">
            {snapshot.sandboxes.length === 0 ? (
              <div className="flex items-center justify-center py-8 text-muted-foreground">
                Немає активних пісочниць
              </div>
            ) : (
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b text-muted-foreground">
                    <th className="text-left py-2 px-3">Функція</th>
                    <th className="text-left py-2 px-3">Статус</th>
                    <th className="text-right py-2 px-3">Cold Start</th>
                    <th className="text-right py-2 px-3">Пам'ять</th>
                    <th className="text-right py-2 px-3">CPU</th>
                    <th className="text-right py-2 px-3">Виклики</th>
                    <th className="text-right py-2 px-3">Egress</th>
                    <th className="text-right py-2 px-3">Блок.</th>
                  </tr>
                </thead>
                <tbody>
                  {snapshot.sandboxes.map((s) => (
                    <tr key={s.instance_id} className="border-b hover:bg-muted/50">
                      <td className="py-2 px-3 font-medium">{s.function_name}</td>
                      <td className="py-2 px-3">
                        <Badge variant="outline" className={statusColors[s.status]}>
                          {s.status}
                        </Badge>
                      </td>
                      <td className="py-2 px-3 text-right font-mono">
                        {s.cold_start_ms > 0 ? `${s.cold_start_ms.toFixed(1)}ms` : "—"}
                      </td>
                      <td className="py-2 px-3 text-right font-mono">
                        {s.memory_used_mb.toFixed(1)}/{s.memory_limit_mb.toFixed(0)}MB
                      </td>
                      <td className="py-2 px-3 text-right font-mono">
                        {s.cpu_percent.toFixed(1)}%
                      </td>
                      <td className="py-2 px-3 text-right font-mono">{s.invocations_total}</td>
                      <td className="py-2 px-3 text-right font-mono">
                        {formatBytes(s.network_egress_bytes)}
                      </td>
                      <td className="py-2 px-3 text-right">
                        {s.egress_blocked > 0 ? (
                          <span className="text-red-500 font-mono">{s.egress_blocked}</span>
                        ) : (
                          <span className="text-muted-foreground">0</span>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </ScrollArea>
        </CardContent>
      </Card>

      {agg.total_egress_blocked > 0 && (
        <Card className="border-red-500/20">
          <CardContent className="p-4 flex items-center gap-3">
            <ShieldAlert className="h-5 w-5 text-red-500" />
            <div>
              <p className="font-medium text-red-600">Hazmat Egress Violations</p>
              <p className="text-sm text-muted-foreground">
                {agg.total_egress_blocked} спроб несанкціонованого мережевого доступу заблоковано
              </p>
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
