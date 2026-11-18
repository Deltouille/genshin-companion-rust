# `wish-ui`: the design system

`crates/wish-ui` is the shared layer (like shadcn's `ui/`): the theme, the typeface, icons, formatting, and the
components every page uses. It depends on GPUI Kit and on `wish-core`'s types only, never on app state: components
take plain data and render it. A piece used by two pages or more goes here; one used by a single page stays in its
feature.

## The look, and the user's choices

**Ask the user which point to change before reworking the look.** These choices were made from screenshots, and
several alternatives were turned down.

- **After Genshin Impact's menus** (the user's choice on 2026-11-12, from the Paimon menu, the character list, the
  inventory and the achievements, and genshinlist.vercel.app), **without any of the game's assets or its font**:
  colours and simple shapes only.
- **Dark**: a plain ink-blue slate page (`#151A26`), slate cards (`#1E2435`) without border, parchment text
  (`#ECE5D8`), the primary a parchment fill with ink text, gold (`#D3BC8E`) for the focus ring and sliders.
- **Light**: a parchment page (`#E8E2D5`) and cards (`#F4F0E7`, a neutral hairline), ink text (`#3B4255`), an ink
  primary, and a slate sidebar as in the Paimon menu (the sidebar uses only the `sidebar.*` tokens).
- **Turned down**: a gradient page and gold borders (on cards, on the current page); then a closer copy of
  genshinlist.vercel.app (a neutral near-black page, warm brown-black panels, every button filled, a slate sidebar
  in both themes) as worse. Earlier looks, all replaced: a muted navy-and-gold one (turned down as AI-generic),
  paimon.moe's purple-navy, a first game-menu "Teyvat" theme, Material 3 after the user's blog.
- **No icons** in page and section titles (2026-11-13). The sidebar's nested pages have **no icon and no ornament**
  (2026-11-17).
- **Item cards** (characters, weapons, materials, wished items) follow the game's: the picture on its rarity tile
  (`stars_tile`: dark top left to light bottom right, the same in both themes) with its bottom right corner rounded
  wide, over a parchment strip with the name or count in ink (`tile_label`). The archive's three lists share one
  card (`item_card`, the user's ask on 2026-11-17).
- **Buttons** are filled parchment pills with ink text in both themes (`button.*` tokens); `.primary()` is
  parchment in dark, ink in light. Pickers are parchment pills over a parchment list; fields, chips and
  `Segmented` are pills.
- **One rule for corners**: every control is a pill (buttons, pickers and their rows, fields, chips, segments,
  sidebar entries, badges); surfaces (cards, panels, the snackbar, a list's frame) use `radius.lg` (12); only item
  tiles keep `radius` (8).
- **Charts**: as few bar charts as possible. Prefer areas, lines with dots, donuts.

## The theme

`crates/wish-ui/src/theme.json` holds every raw colour, light and dark; nothing else does. The kit's tokens are
there, and the app's own roles (parchment, ink, gold, the 1★–5★ tiles) are each theme's `app` section, which the kit
ignores and `theme::palette(cx)` reads. `accent` is the hover state layer, drawn over a surface rather than in its
place.

- Rarity text colours: 5★ `#FFB13F` / `#965700`, 4★ `#D28FD6` / `#8E3F94` (dark / light); always beside the written
  rarity ("5★"), never the only cue.
- Keep text contrast at 4.5:1. When surfaces change, rerun the dataviz validator on the chart colours.
- `theme.rs` registers Google Sans Flex (`crates/wish-ui/fonts/`, SIL OFL): five static instances of the variable
  font (fonttools' `varLib.instancer` at opsz 18, wdth 100, ROND 0), since GPUI can't vary a variable font's weight
  and the default font lacks bold and black. `follow_system` switches with the system's light or dark.

## Components

`crates/wish-ui/src/components/`. GPUI Kit's controls are 32 px with 14 px text, which looks like another app next
to these proportions: **never use `Button::new`, `Select::new`, `Input::new`, or the kit's `Pagination`,
`ButtonGroup` and `Toggle` directly in the app.**

| Need | Use | Not |
| --- | --- | --- |
| a button | `button(id)` (+ `.primary()`, `.ghost()`, `.danger()`, `.label()`, `.icon()`) | `Button::new` |
| an icon-only button | `icon_button` (with an accessibility label and a tooltip) | |
| a button the kit builds itself (a dialog's close) | `button_style(…)` | |
| a disabled button | `.unavailable(bool)` (`ButtonExt`): it also gives the arrow back | the kit's `.disabled(…)` |
| a choice in a list | `picker` with a `PickerState` (search for long lists, Up/Down/Enter) | the kit's `Select` |
| a text field | `input(&state, cx)` | `Input::new` |
| a few views or values, one on | `Segmented` | `TabBar`, `ButtonGroup`, `Toggle` |
| an on/off filter | `filter_chip` | `Toggle` |
| pages of a table | `Pager` | `Pagination` |
| a dialog's buttons | `dialog_footer(button(…).primary() / .danger())`, `dialog_close_footer` | the kit's built-in OK/Cancel |
| a page's frame | `PageFrame` (`.max_width` centres it in a column) | |
| a surface and its title | `card`, `card_title` | |
| a sidebar entry, a feature's group | `NavItem` (`nested` for a feature's pages), `NavGroup` | the kit's `SidebarMenuItem` (28 px) |
| a material and its count | `MaterialTile` (`.done`, `bare`, `.crafted(n)`) | |
| an item's icon on its tile | `item_tile` | |
| figures | `stat`, `stat_tile`, `big_number`, `stat_label` | |
| labels | `badge`, `outcome_badge` | |
| game text with highlighted terms | `rich_text` (`**…**` turns semibold) | |
| a message about background work | `snackbar` (drawn by the app's notices) | `window.push_notification` |

**Everything clickable shows the pointing hand.** These helpers do it, since the kit gives every button but a link
the arrow. Anything else clickable (a `div` with `on_click`, a kit `Slider` or `Switch`) takes `.cursor_pointer()`
itself; a button that does nothing (the current tab) takes `.cursor_default()`.

## Charts

- Series colours come from `theme::rarity_chart` (`chart.2`/`.3`/`.4` in `theme.json`, validated with the dataviz
  script against both card surfaces), not the text colours of `theme::rarity`. `theme.chart_2` (`#4E7CFF`) passes on
  both surfaces.
- `axis.rs` picks tick counts and axis tops so a counting chart's ticks stay whole (the kit spaces ticks evenly from
  zero, so a maximum of 5 would label 1.25 as "1").
- `legend` names a chart's series; `pity_chart` (each 5★'s pity as a line of dots with the average dashed) is shared
  by the Overview and the history page.
- Give a chart card a definite height and its chart `flex_1().min_h_0()`, never a `min_h` larger than its row.
  Offscreen captures need `.appear(false)` or charts draw empty.

## Formatting and icons

- `format.rs`: numbers, sizes, dates, quotes, servers, weekdays, `capitalize`, the game's fixed terms, in the
  current language ([i18n.md](i18n.md)).
- `assets.rs`: `UiAssets`, GPUI Kit's component icons plus the Lucide icons listed in `icon_assets!` (only those are
  compiled in); `pool_icon`. Only the icons in `gpui-kit-assets`' `default-icons.txt` are bundled (there is no
  `trash`; `delete` is a backspace glyph): add others to `icon_assets!` and use them as
  `gpui_kit::assets::IconName::…`. Lucide's licence is in the third-party licences file.

## Seeing it

`cargo run -p wish-ui --example gallery -- <out-dir>` renders every component offscreen, light and dark. The app's
pages: the screenshot tool ([testing.md](testing.md)).

Before UI work, read the GPUI Kit skills' guides (`gpui-kit-design-guides`, and the Coding Guides in `gpui-kit`).
