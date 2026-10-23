# RE Rematch

A study of the gameplay mechanics of Rematch (Sloclap, Unreal Engine 5), for a clean-room reimplementation in Rust.

**Rules:**
- Only values and observed behaviour go in this repo.
- No decompiled code, and no game assets (models, animations, textures, sounds).

## Notes
| File | Contents |
|---|---|
| [notes/mechanics.md](notes/mechanics.md) | stamina, movement speeds, cooldowns, shooting, volleys, dribbling, tackles, dash, dives, extra effort, ball physics |
| [notes/controls.md](notes/controls.md) | Rematch's default keyboard and gamepad bindings |
| [notes/frame-data.md](notes/frame-data.md) | cancel windows and when you can act again after every action |
| [notes/frame-data.csv](notes/frame-data.csv) | the same, per animation (766 rows, 60 fps frames) |
| [notes/hitboxes.md](notes/hitboxes.md) | reception zones, tackle and dive hitboxes, pitch sizes |
| [notes/arena.md](notes/arena.md) | walls, corners, ceiling, goal size, goalkeeper and sweeper areas for each pitch size |
| [notes/match-camera-network.md](notes/match-camera-network.md) | match rules, camera settings, network interaction priority |
| [notes/progression.md](notes/progression.md) | contracts and badges |
| [notes/bots.md](notes/bots.md) | how the bot AI decides (state trees, positioning queries) |
| `notes/asset-list.txt` | the 41,176 asset paths in the game's IoStore container |

## Tools
- `tools/usmapgen.py`: builds the `.usmap` mappings file from the static reflection tables in the game executable, without running the game.
  - `python -I tools/usmapgen.py <RuntimeClient-Win64-Shipping.exe> tools/rmx/Rematch.usmap`
  - Output on the current build: 5140 classes, ~5500 structs, 871 enums.
- `tools/oodle-shim`: builds a decompression-only `oo2core_9_win64.dll` from the open-source [ooz](https://github.com/powzix/ooz) decoder (GPL-3) with MinGW `g++`.
  - `sh tools/oodle-shim/build.sh <short-temp-dir>`, then copy the DLL into `tools/rmx/`.
- `tools/rmx`: a CUE4Parse (.NET 8) reader.
  - `pwsh -File tools/rmx/run.ps1` reads the asset paths in `targets.txt` and writes JSON to `out/`.
  - `run.ps1 --list` dumps the asset list instead.

## Game facts used by the tools
- **Containers:** the IoStore container (`Runtime-WindowsClient.utoc/.ucas`, 11 GB) is not encrypted. The `.pak` (2.2 GB) has an encrypted index and is not needed.
- **Mappings:** assets use unversioned properties, so they need the generated `.usmap`.
- **Compression:** most assets are Oodle-compressed.
- **Frame rate:** gameplay runs at 60 fps. Verified against the standing tackle animation: hit at 0.1167 s = frame 7.
- **Anti-cheat:** the game uses Easy Anti-Cheat. Everything here works on the files at rest; nothing touches the running game.
