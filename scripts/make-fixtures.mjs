// Builds anonymised fixtures from real session files: keeps only the fields Tally reads.
import fs from "node:fs";
import path from "node:path";
import os from "node:os";

const [claudeFile, codexFile] = process.argv.slice(2);
const out = path.resolve(import.meta.dirname, "..", "fixtures");
const anonCwd = "C:/Users/dev/projects/demo";

const claudeLines = fs.readFileSync(claudeFile, "utf8").split("\n").filter(Boolean);
const keptClaude = [];
let users = 0;
for (const raw of claudeLines) {
  const l = JSON.parse(raw);
  if (l.type === "user" && users < 1) {
    users++;
    keptClaude.push({ type: "user", timestamp: l.timestamp, cwd: anonCwd, message: { role: "user", content: "hello" } });
  }
  if (l.type === "assistant" && l.message?.usage) {
    keptClaude.push({
      type: "assistant", timestamp: l.timestamp, cwd: anonCwd, requestId: l.requestId,
      message: { id: l.message.id, model: l.message.model, role: "assistant", usage: {
        input_tokens: l.message.usage.input_tokens, output_tokens: l.message.usage.output_tokens,
        cache_read_input_tokens: l.message.usage.cache_read_input_tokens ?? 0,
        cache_creation_input_tokens: l.message.usage.cache_creation_input_tokens ?? 0 } },
    });
  }
  if (keptClaude.length >= 12) break;
}
const claudeOut = keptClaude.map((l) => JSON.stringify(l));
claudeOut.push("{not valid json");
claudeOut.push(JSON.stringify({ type: "summary", summary: "unknown line type" }));
fs.writeFileSync(path.join(out, "claude-code", "session.jsonl"), claudeOut.join("\n") + "\n");

const codexLines = fs.readFileSync(codexFile, "utf8").split("\n").filter(Boolean);
const keptCodex = [];
let records = 0, counts = 0, ctx = 0;
for (const raw of codexLines) {
  const l = JSON.parse(raw);
  if (l.type === "session_meta") keptCodex.push({ timestamp: l.timestamp, type: "session_meta", payload: { id: "00000000-0000-0000-0000-000000000001", cwd: anonCwd } });
  if (l.type === "turn_context" && ctx < 1) { ctx++; keptCodex.push({ timestamp: l.timestamp, type: "turn_context", payload: { cwd: anonCwd, model: l.payload.model } }); }
  if (l.type === "token_usage_record" && records < 4) { records++; keptCodex.push({ timestamp: l.timestamp, type: "token_usage_record", payload: { response_id: l.payload.response_id, usage: l.payload.usage } }); }
  if (l.type === "event_msg" && l.payload?.type === "token_count" && counts < 4) { counts++; keptCodex.push({ timestamp: l.timestamp, type: "event_msg", payload: { type: "token_count", info: l.payload.info } }); }
  if (records >= 4 && counts >= 4) break;
}
fs.writeFileSync(path.join(out, "codex", "rollout-with-records.jsonl"), keptCodex.map((l) => JSON.stringify(l)).join("\n") + "\n");
// Older Codex versions only emit token_count events; repeat one to exercise dedup.
const legacy = keptCodex.filter((l) => l.type !== "token_usage_record");
const firstCount = legacy.find((l) => l.type === "event_msg");
legacy.push(firstCount);
fs.writeFileSync(path.join(out, "codex", "rollout-legacy.jsonl"), legacy.map((l) => JSON.stringify(l)).join("\n") + "\n");
console.log("claude", keptClaude.length, "codex", keptCodex.length, os.EOL);
