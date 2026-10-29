# Ajouter un agent à Tally

Un agent, c'est un **provider** : un fichier Rust qui lit les journaux de session de l'agent et en sort des `UsageEvent`. Le reste de l'app (base SQLite, watcher, totaux, UI) ne change pas.

Compte une heure pour un agent qui écrit du JSONL.

## 1. Trouver les fichiers

Repère où l'agent écrit ses sessions sur Windows et ce qu'une ligne contient. Il te faut au minimum :

- un horodatage (RFC 3339) ;
- les compteurs de tokens (input, output, et le cache s'il existe) ;
- un identifiant stable par réponse, pour dédoublonner si la même ligne revient.

Le modèle et le dossier de travail sont des bonus, l'app les affiche s'ils sont là.

## 2. Ajouter des fixtures

Copie une vraie session dans `fixtures/<id-agent>/`, puis **nettoie-la** : remplace les prompts, les réponses et les chemins personnels par du texte neutre. Garde les compteurs tels quels.

Ajoute exprès quelques cas difficiles : une ligne en double, une ligne JSON cassée, une ligne sans usage.

## 3. Écrire le provider

Crée `src-tauri/src/providers/<id_agent>.rs` :

```rust
use super::{home, num, parse_ts, project_name, read_lines, Provider, UsageEvent};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct MonAgent;

impl Provider for MonAgent {
    fn id(&self) -> &'static str { "mon-agent" }
    fn display_name(&self) -> &'static str { "Mon Agent" }

    fn default_paths(&self) -> Vec<PathBuf> {
        vec![home().join(".mon-agent").join("sessions")]
    }

    fn parse_file(&self, path: &Path, from_offset: u64) -> Result<(Vec<UsageEvent>, u64)> {
        let (lines, offset) = read_lines(path, from_offset)?;
        let events = lines.iter().filter_map(|line| {
            let v: serde_json::Value = serde_json::from_str(line).ok()?;
            let usage = v.get("usage")?;
            Some(UsageEvent {
                id: v.get("id")?.as_str()?.to_string(),
                agent: self.id().to_string(),
                model: v.get("model").and_then(|m| m.as_str()).map(str::to_string),
                timestamp: parse_ts(v.get("timestamp")?)?,
                input: num(usage, "input_tokens"),
                output: num(usage, "output_tokens"),
                cache_read: num(usage, "cache_read_tokens"),
                cache_write: 0,
                project: project_name(v.get("cwd").and_then(|c| c.as_str())),
            })
        }).collect();
        Ok((events, offset))
    }
}
```

Les helpers du module `providers` font le gros du travail :

| Helper | Rôle |
| --- | --- |
| `read_lines(path, offset)` | Lit les lignes complètes depuis `offset` et laisse de côté une dernière ligne pas encore finie. |
| `parse_ts(value)` | Convertit un horodatage RFC 3339 en `DateTime<Utc>`. |
| `num(value, key)` | Lit un compteur, 0 s'il manque. |
| `project_name(cwd)` | Garde le nom du dernier dossier comme projet. |

Règles :

- **Lecture seule.** Ne modifie jamais les fichiers de l'agent.
- **Ne plante jamais sur une ligne.** Une ligne illisible est ignorée.
- **`id` unique par agent.** La base dédoublonne dessus, donc relire un fichier ne compte rien deux fois.
- **Input sans le cache.** Si l'agent inclut le cache dans l'input, soustrais-le.

## 4. L'enregistrer

Dans `src-tauri/src/providers/mod.rs` :

```rust
mod mon_agent;
pub use mon_agent::MonAgent;

pub fn registry() -> Vec<Box<dyn Provider>> {
    vec![Box::new(ClaudeCode), Box::new(Codex), Box::new(MonAgent)]
}
```

Côté front, ajoute sa couleur et son nom dans `src/lib/agents.ts`.

## 5. Tester

Ajoute un module `tests` dans ton fichier, sur le modèle de `claude_code.rs` :

- les compteurs de la première ligne sont les bons ;
- les doublons partagent le même `id` ;
- une reprise depuis un offset ne relit rien.

```bash
cd src-tauri
cargo test
```

Puis lance l'app avec `npm run tauri dev` et vérifie que l'agent apparaît dans la vue Agents.

## 6. Ouvrir la PR

Dis dans la description où l'agent écrit ses fichiers et avec quelle version tu as testé.
