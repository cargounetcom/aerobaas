import { useAuditLog } from "@/hooks/use-audit-log";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Bot, Shield, User, AlertTriangle, Activity, Cpu } from "lucide-react";

const actorIcons: Record<string, typeof User> = {
  user: User,
  ai_agent: Bot,
  system: Activity,
  hazmat_module: Shield,
  remediation_module: Shield,
  erlang_orchestrator: Cpu,
};

const severityColors: Record<string, string> = {
  info: "bg-blue-500/10 text-blue-600 border-blue-500/20",
  warning: "bg-yellow-500/10 text-yellow-600 border-yellow-500/20",
  critical: "bg-red-500/10 text-red-600 border-red-500/20",
  security: "bg-purple-500/10 text-purple-600 border-purple-500/20",
};

const actionLabels: Record<string, string> = {
  login: "Вхід в систему",
  logout: "Вихід",
  register: "Реєстрація",
  workspace_created: "Створення робочого простору",
  workspace_deleted: "Видалення робочого простору",
  member_invited: "Запрошення учасника",
  member_removed: "Видалення учасника",
  member_role_changed: "Зміна ролі учасника",
  key_created: "Створення API-ключа",
  key_deleted: "Видалення API-ключа",
  key_revoked: "Відкликання API-ключа",
  function_deployed: "Деплой функції",
  function_deleted: "Видалення функції",
  function_executed: "Виконання функції",
  function_blocked: "Блокування функції",
  collection_created: "Створення колекції",
  collection_deleted: "Видалення колекції",
  scan_completed: "Сканування завершено",
  remediation_applied: "Авто-виправлення застосовано",
  remediation_blocked: "Деплой заблоковано (Remediation)",
  hazmat_violation: "Порушення Hazmat",
  hazmat_quarantine_passed: "Hazmat карантин пройдено",
  ai_generated: "ІІ згенерував код",
  ai_chat_message: "Повідомлення ІІ-чату",
  skill_installed: "Навичку встановлено",
  skill_deployed: "Навичку розгорнуто",
  workspace_switched: "Перемикання робочого простору",
  anomaly_detected: "Аномалію виявлено",
  credits_consumed: "Кредити списано",
  credits_exhausted: "Кредити вичерпано",
};

function formatTime(timestamp: string): string {
  const date = new Date(timestamp);
  return date.toLocaleTimeString("uk-UA", { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

export function AuditLogPage() {
  const { events, loading, isConnected, filter, setFilter } = useAuditLog();

  return (
    <div className="container mx-auto py-6 space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Realtime Audit Log</h1>
          <p className="text-muted-foreground">Жива стрічка дій ІІ-агентів та команди</p>
        </div>
        <div className="flex items-center gap-2">
          {isConnected ? (
            <Badge className="bg-green-500/10 text-green-600 border-green-500/20">
              <span className="mr-1 h-2 w-2 rounded-full bg-green-500 animate-pulse" />
              Live
            </Badge>
          ) : (
            <Badge variant="outline">Disconnected</Badge>
          )}
        </div>
      </div>

      <div className="flex gap-2">
        {["all", "user", "ai_agent", "system", "hazmat_module"].map((type) => (
          <button
            key={type}
            onClick={() => setFilter((prev) => ({ ...prev, actor_type: type === "all" ? undefined : type }))}
            className={`px-3 py-1 rounded-md text-sm border transition-colors ${
              (filter.actor_type || "all") === type
                ? "bg-primary text-primary-foreground"
                : "bg-background hover:bg-muted"
            }`}
          >
            {type === "all" ? "Всі" : type === "ai_agent" ? "ІІ-Агенти" : type === "hazmat_module" ? "Hazmat" : type === "system" ? "Система" : "Користувачі"}
          </button>
        ))}
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Події ({events.length})</CardTitle>
        </CardHeader>
        <CardContent>
          <ScrollArea className="h-[600px]">
            {loading ? (
              <div className="flex items-center justify-center py-8 text-muted-foreground">
                Завантаження...
              </div>
            ) : events.length === 0 ? (
              <div className="flex items-center justify-center py-8 text-muted-foreground">
                Подій поки що немає
              </div>
            ) : (
              <div className="space-y-2">
                {events.map((event) => {
                  const ActorIcon = actorIcons[event.actor_type] || Activity;
                  return (
                    <div
                      key={event.id}
                      className="flex items-start gap-3 p-3 rounded-lg border hover:bg-muted/50 transition-colors"
                    >
                      <div className="mt-0.5 shrink-0">
                        <ActorIcon className="h-4 w-4 text-muted-foreground" />
                      </div>
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2 flex-wrap">
                          <span className="text-xs text-muted-foreground font-mono">
                            {formatTime(event.timestamp)}
                          </span>
                          <Badge variant="outline" className={severityColors[event.severity]}>
                            {event.severity}
                          </Badge>
                          <span className="text-sm font-medium">
                            {actionLabels[event.action] || event.action}
                          </span>
                        </div>
                        <div className="mt-1 text-sm text-muted-foreground">
                          <span className="font-medium">{event.actor_name}</span>
                          {" · "}
                          <span>{event.resource_type}</span>
                          {event.resource_id && (
                            <span className="font-mono text-xs"> · {event.resource_id.slice(0, 8)}...</span>
                          )}
                        </div>
                        {event.details && Object.keys(event.details).length > 0 && (
                          <pre className="mt-1 text-xs text-muted-foreground/70 overflow-x-auto">
                            {JSON.stringify(event.details, null, 2)}
                          </pre>
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </ScrollArea>
        </CardContent>
      </Card>
    </div>
  );
}
