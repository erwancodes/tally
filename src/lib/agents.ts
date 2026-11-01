/** Display name and series colour per provider id. Unknown agents get a neutral tone. */
export const AGENTS: Record<string, { name: string; color: string }> = {
  "claude-code": { name: "Claude Code", color: "var(--claude)" },
  codex: { name: "Codex", color: "var(--codex)" },
  opencode: { name: "OpenCode", color: "var(--opencode)" },
  kimi: { name: "Kimi Code", color: "var(--kimi)" },
  grok: { name: "Grok CLI", color: "var(--grok)" },
  cursor: { name: "Cursor", color: "var(--cursor)" },
};

export const agentName = (id: string) => AGENTS[id]?.name ?? id;
export const agentColor = (id: string) => AGENTS[id]?.color ?? "var(--ink-3)";
