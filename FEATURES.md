# Time Tracker — supported features (from TimeTracker-master)

Reference implementation: Electron + React app in `TimeTracker-master/`.

## Work items

| Feature | Description |
|---------|-------------|
| Add item | Create a work item on a chosen calendar day |
| Edit item | Open form from day panel; title required |
| Delete item | Remove item from that day |
| Fields | Title, optional description, category, achievement flag |
| Categories | Development, Code Review, Meetings, DevOps, Documentation, Other |
| Achievement | Star/badge on item; highlighted in exports |
| Copy to another day | Duplicate item to a new date; original unchanged |
| Auto-save | Persist items after every change (desktop: `~/.weekly-tracker/data.json`) |

## Calendar & navigation

| Feature | Description |
|---------|-------------|
| Month grid | Mon–Sun layout with ISO week numbers in the first column |
| Month navigation | Previous / next month, **Today** resets to current month |
| Leading/trailing days | Days outside current month shown muted |
| Today highlight | Current day emphasized on the grid |
| Current week row | Background highlight for the week containing today |
| Day indicators | Dot density by item count (1/2/3+); amber dot if any achievement |
| Day click (pin) | Toggle pinned day; when unpinned, detail follows hover |
| Day hover | Preview that day’s items in the side panel |

## Export

| Feature | Description |
|---------|-------------|
| Date range | From / to with validation |
| Presets | This week, last 7 days, last 30 days, this month |
| Preview | Report structure before export |
| Markdown download | Save `.md` via system dialog; open file after save |
| Report contents | Achievements section + per-day work items |
| PDF (original) | html2pdf in JS app — not ported in Rust v0.1 (export MD instead) |

## Appearance

| Feature | Description |
|---------|-------------|
| Themes | Light (default), Dark, Warm, Forest, Ocean, Rose |
| Theme persistence | Stored in `prefs.json` |
| Redesigned palette (Rust) | ui-ux-pro-max: slate background, blue primary, emerald accent |

## Settings & app shell

| Feature | Description |
|---------|-------------|
| Storage tab | Explains local `~/.weekly-tracker/` layout |
| Updates tab | Check on startup (optional), manual GitHub release check |
| Update banner | Shown under toolbar when newer release detected |
| About | Name, version, tagline, GPL description |
| Contact | Email, GitHub repo / issues |
| Splash screen | Short intro on launch (original) |
| Footer | Copyright, license, app name + version |
| Open external | GitHub HTTPS links only (original Electron guard) |

## Data model (per item)

```json
{
  "id": "uuid",
  "date": "yyyy-MM-dd",
  "title": "...",
  "description": "...",
  "category": "development",
  "isAchievement": false,
  "createdAt": "ISO-8601",
  "updatedAt": "ISO-8601"
}
```

## Rust port status

Implemented in `weekly_work_tracker`: calendar (grid styling), CRUD, copy, themes, export MD, settings/about, splash screen, GitHub release check + banner, compatible data paths. PDF export deferred.