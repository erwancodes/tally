# Tally

**Tes tokens IA, juste à côté de l'heure.**

Tally vit dans la barre des tâches de Windows et compte les tokens que tu consommes avec tes agents IA de code, par jour, par semaine et par mois. Tout est lu en local : aucune API, aucune clé, aucun compte.

[Télécharger pour Windows](https://github.com/erwancodes/tally/releases/latest) · [tally.erwanx.com](https://tally.erwanx.com)

![Tally](landing/og.png)

## Ce qu'il fait

- Une icône dans la zone de notification, un tooltip `Aujourd'hui : 1,2M tokens` au survol.
- Un clic ouvre la fenêtre : aujourd'hui, 7 jours, mois en cours, graphe des 30 derniers jours, détail par agent et par modèle.
- Input, output, cache lu et cache écrit, avec un réglage pour compter le cache ou non.
- Mise à jour en direct quand un agent écrit.
- Lancement au démarrage de Windows, en option.

## Agents

| Agent | Fichiers lus | État |
| --- | --- | --- |
| Claude Code | `~\.claude\projects\**\*.jsonl` | Pris en charge |
| Codex CLI | `~\.codex\sessions\**\rollout-*.jsonl` | Pris en charge |
| OpenCode | | Bientôt |
| Kimi Code | `~\.kimi\sessions\**\wire.jsonl` | Pris en charge |

Ton agent n'y est pas ? [Ajoute-le](docs/ADD_A_PROVIDER.md), c'est un fichier Rust.

## Installer

Télécharge le `.msi` ou le `.exe` de la [dernière release](https://github.com/erwancodes/tally/releases/latest) et lance-le. Windows 10 ou 11.

## FAQ

**Mes données sortent-elles de mon PC ?**
Non. Tally lit les journaux que tes agents écrivent déjà sur ton disque et garde seulement des compteurs dans `%APPDATA%\Tally\tally.db`. L'app ne fait aucune requête réseau et n'a aucune télémétrie.

**Tally modifie-t-il les fichiers de mes agents ?**
Non, il les lit seulement.

**macOS et Linux ?**
Pas encore. Tally est construit avec Tauri, donc c'est possible plus tard.

## Développer

Il te faut Node 22+, Rust stable et les [prérequis Tauri pour Windows](https://v2.tauri.app/start/prerequisites/).

```bash
npm install
npm run tauri dev
```

Tests :

```bash
npm test
cd src-tauri && cargo test
```

Build de release (`.msi` et `.exe` dans `src-tauri/target/release/bundle/`) :

```bash
npm run tauri build
```

Hors de Tauri (`npm run dev`), le front affiche des données de démo.

### Structure

```
src/              front React + TypeScript + Tailwind
src-tauri/src/
  providers/      un fichier par agent
  store.rs        SQLite, lecture incrémentale, dédoublonnage
  watcher.rs      surveillance des dossiers des agents
  tray.rs         icône, tooltip et menu
fixtures/         sessions nettoyées pour les tests
landing/          tally.erwanx.com, page statique
```

## Licence

MIT, voir [LICENSE](LICENSE). Fait par [@ErwanCodes](https://x.com/ErwanCodes).
