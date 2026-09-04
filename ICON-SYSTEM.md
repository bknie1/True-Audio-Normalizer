# TAN icon system

```
      .-.      .-.      .-.
     /   \    /   \    /   \
    /     '--'     '--'     \      thin line, one weight,
   '                         '     currentColor, no icon fonts
```

How icons are drawn, sized, colored and labeled on the TAN portal
(`docs/index.html`, `docs/sound-journey.html`), and why. The trigger was
the six-button song picker on the sound journey page: it shipped as
icon-only squares with `title` and `aria-label` and nothing visible to
read. That passes an automated checker and still fails most of the people
it was supposed to help. Part 1 explains what "accessible icon UI"
actually requires, with the WCAG criteria that back each claim. Part 2 is
the spec, ending in corrected code for one song button.

Nothing here adds a dependency. The portal has no icon font and no CDN,
on purpose, and the spec keeps it that way: every icon is inline SVG.

## Part 1: what an icon-only control is missing

### Four audiences, one button

A button that is only a picture has to work for four different people at
once. `aria-label` reaches exactly one of them.

Screen reader users hear the accessible name. `aria-label` gives them
one. They are covered.

Low-vision users who zoom to 200 to 400 percent, or run high-contrast
mode, or just have aging eyes, do not use a screen reader. They see the
picture and nothing else. A dollar sign at 22 px tells them nothing about
which of six songs it plays. `aria-label` is invisible to them.

Voice-control users (Dragon, Windows Voice Access, Apple Voice Control)
click by speaking the control's name. The name comes from the
accessibility tree, so `aria-label` technically works, except they cannot
see what the name is, so they cannot say it. They end up saying "show
numbers" and picking a grid overlay, which is the fallback for broken UI.

Everyone else, sighted mouse and touch users included, has to guess what
the icon means. A dollar sign, a shield, a target, a star, an hourglass, a
heart: none of those is a convention for a song title. They are visual
jokes that only land after you already know the answer. Touch users never
see a `title` tooltip at all, so they have no way to find out short of
tapping each one.

### What `aria-label` does and does not do

It does: set the accessible name in the accessibility tree, which screen
readers announce and voice-control software matches against. Nothing
else.

It does not: render anything, help anyone who is not running assistive
technology, explain an unfamiliar icon, appear on hover or touch, or
reliably survive browser translation (attribute text is translated
inconsistently across browsers). If its text differs from any visible
text on the control, it also breaks SC 2.5.3 Label in Name, because voice
users say what they see and the software matches against what you typed.

Rule of thumb: `aria-label` is the right tool for naming something that
already has a visible meaning (a close X, a play triangle). It is the
wrong tool for giving meaning to something that has none.

### Why `title` is not a label

`title` produces a native tooltip after roughly a one second hover with a
mouse. It never appears on keyboard focus in most browsers, never appears
on touch, cannot be hovered onto, cannot be dismissed with Escape, and on
some platforms disappears on its own after a few seconds. Screen readers
use it as an accessible name only when nothing else names the element;
when `aria-label` is also present, most of them read the same text twice
(once as name, once as description), which is what the original picker
did.

The precise WCAG position matters here because people misquote it. SC
1.4.13 Content on Hover or Focus (AA) has an explicit exception for
content whose "visual presentation is controlled by the user agent and is
not modified by the author." Native `title` tooltips fall under that
exception, so they do not fail 1.4.13. They are exempt because the author
has no control over them, and that lack of control is exactly why they
are useless to keyboard and touch users. The moment you replace `title`
with a custom tooltip, 1.4.13 applies in full and demands three things:
dismissible (Escape closes it without moving focus), hoverable (the
pointer can move from the trigger onto the tooltip without it vanishing),
and persistent (it stays until hover or focus leaves or the user
dismisses it). Part 2 builds a tooltip that does all three.

### The success criteria, and how the original picker scored

SC 1.1.1 Non-text Content (A). Every non-text element that conveys
information needs a text alternative; decorative ones must be hidden
from assistive technology so they are not announced as "image" or
"graphic". Original picker: passed on paper via `aria-label`, but the
SVGs had no `aria-hidden="true"`, so some screen readers announced both
the name and an empty graphic.

SC 1.4.11 Non-text Contrast (AA). The parts of a control needed to
identify it and its state, and any graphic needed to understand content,
must have a contrast ratio of at least 3:1 against adjacent colors. This
is the one almost everyone misses, because 4.5:1 for text is the number
people remember and icons are not text. Measured against the actual
portal tokens (see the table in Part 2), the music row accent `#c98a2e`
is 2.71:1 on the light background and 2.93:1 on white. The selected song
button drew its icon in that accent on light theme at 2.51:1. Fail.

SC 1.4.1 Use of Color (A). State cannot be shown by hue alone. The
original selected state was a 16 percent tint plus a hue change on the
icon. The tint is a luminance change, so it is technically not "color
alone", but at 1.1:1 against the card it is close to invisible on a
phone in daylight. Weak pass; Part 2 adds a bolder second cue.

SC 2.5.8 Target Size (Minimum) (AA, WCAG 2.2). Each target is at least
24 by 24 CSS px, or is spaced so a 24 px circle centered on it overlaps no
other target. SC 2.5.5 Target Size (Enhanced) (AAA, WCAG 2.1) asks for
44 by 44. The picker buttons are 76 by 64 with a 0.6 rem gap. Pass at
AAA. Keep it that way; this is the cheapest criterion to meet and the
most painful to fix later.

SC 2.4.7 Focus Visible (AA), SC 2.4.11 Focus Not Obscured (Minimum) (AA,
2.2), SC 2.4.13 Focus Appearance (AAA, 2.2). The focused control must
have a visible indicator; 2.4.13 wants a 2 px perimeter with 3:1 contrast
between focused and unfocused states. The picker uses a 2 px outline in
the row accent. On the music row in light theme that outline is 2.71:1
against the page. Fail at AAA, marginal at AA. Part 2 moves the focus
ring to `--ink`.

SC 2.5.3 Label in Name (A). If a control has visible text, its
accessible name must contain that text. Irrelevant when there is no
visible text; becomes load-bearing the moment a label is added. The
simplest way to comply is to let the visible text be the name and delete
`aria-label`.

SC 4.1.2 Name, Role, Value (A). Controls must expose their name, role and
current state. Six mutually exclusive choices is a radio group, and
"which one is selected" is state. The original used plain buttons with a
CSS class for the selected one, so a screen reader user heard six
identical-sounding buttons and had no way to know which song was
current. Fail.

SC 1.4.3 Contrast (Minimum) (AA). Once labels exist they are text and
need 4.5:1. The 0.68 rem labels in `--muted` measure 5.01:1 light and
6.16:1 dark. Pass.

### What the research says beyond compliance

The usability case is older and blunter than the WCAG one.

Nielsen Norman Group's "Icon Usability" (Aurora Harley, 2014) is the
standard reference: users can recognize an icon's shape and still not
know what it does, universally understood icons are rare (home, print,
search, close, play, and not many more), and a visible text label is the
fix, with the icon acting as a scanning aid rather than the carrier of
meaning. The same article gives the five second test: if a first-time
user cannot say what an icon does within five seconds, it needs a label.

NN/g's "Tooltip Guidelines" (2019) adds that tooltips are for
supplementary detail on something already labeled, never for the label
itself, because they are invisible until discovered and unavailable on
touch.

Vincent Flanders coined "mystery meat navigation" in 1998 for exactly
this pattern: controls whose meaning is only revealed on hover. The term
stuck because the problem never went away.

The cognitive cost is the part that survives even a perfect ARIA
implementation. Every unfamiliar icon is a small memory test. Six of them
in a row, all abstract, is six tests before the user gets to hear any
music. The page exists to demonstrate an audio engine; the picker should
cost nothing.

## Part 2: the spec

### Two sizes, one stroke feel

Everything already on the portal falls into two families. The spec names
them and locks the numbers.

Glyph: a 24-unit viewBox, drawn inside a 20-unit safe area (2 units of
margin on every side), `stroke-width="1.8"`, `stroke-linecap="round"`,
`stroke-linejoin="round"`, `fill="none"` except for dots of radius 1 or
less. Rendered between 18 and 24 CSS px, never below 16. The stroke lands
at about 1.6 px on screen. This is the song picker, and any future inline
or button icon.

Art: rendered 84 to 220 px, stroke lands at about 3.3 px on screen. Two
grids already exist and both are fine: the 100-unit feature icons on
`index.html` use `stroke-width="4"`; the 200-unit row art on
`sound-journey.html` uses `stroke-width="3"`. New art icons pick
whichever grid the page already uses. Art may use a second stroke at
`opacity="0.3"` to `"0.5"` for a ghosted echo (the existing feature icons
do this) and small solid fills for emphasis points.

Nothing between the two sizes. A 48 px icon reads as neither and will
look like it came from somewhere else.

Drawing rules for both families: single stroke weight per icon; no
gradients, no shadows inside the SVG (glow lives on the container, see
`.row .art::before`); no text inside the SVG; geometry snaps to half
units so strokes do not blur; and the icon must survive being drawn in
one flat color, because that is how it will be seen in forced-colors
mode.

### Color

Glyphs use `stroke="currentColor"` and never a literal hex. The element
around them sets `color`, and that element's color has already been
chosen to pass text contrast, so the glyph passes 3:1 by inheritance.
This is the whole trick: an icon that inherits its color from readable
text cannot fail 1.4.11.

Art uses the row or feature accent as a literal hex, matching the
existing files, because art is decorative and sits on its own glow.
Decorative means it must carry `aria-hidden="true"` and the meaning must
be in the adjacent heading. Every current art icon already sits next to
one.

Accents may color a glyph only in a state that also changes something
else (border, weight), and only if the accent passes 3:1 against the
surface in that theme. Measured ratios for the tokens in `:root`:

```
                         light theme            dark theme
                         bg      card           bg      card
                         #f7f6f3 #ffffff        #161513 #201f1c
--ink                    15.8    17.1           14.6    13.2
--muted                   5.0     5.4            6.2     5.6
--accent (site)           4.6     4.9            6.0     5.4
music   #c98a2e           2.7     2.9            6.2     5.6     FAIL light
movie   #3f9179           3.5     3.8            4.8     4.4
game    #5661c4           5.0     5.4            3.4     3.1     marginal dark
podcast #3d7ab5           4.2     4.5            4.0     3.7
```

The site accent (`#b3552e` light, `#e0764a` dark) was already
theme-split and passes both. Two of the four row accents were not split
and one fails. The fix is to split them the same way the site accent is
split, in a class per row instead of an inline `style`:

```css
.row.music   { --accent-row: #946418; }   /* 4.7 on bg, 5.1 on card */
.row.movie   { --accent-row: #3f9179; }
.row.game    { --accent-row: #5661c4; }
.row.podcast { --accent-row: #3d7ab5; }
@media (prefers-color-scheme: dark) {
  .row.music { --accent-row: #c98a2e; }   /* the current value, fine on dark */
  .row.game  { --accent-row: #7b85e3; }   /* 5.5 on bg, 5.0 on card */
}
```

The aurora script reads `--accent-row` through `getComputedStyle`, so it
picks up the class values without changes. Light theme's music glow
shifts a little browner. That is the correct trade.

Related finding, outside the icon scope but caught by the same numbers:
the filled `Run through TAN` buttons draw white text on the row accent.
White on `#c98a2e` is 2.93:1 and fails text contrast. With the split
above, white on light-theme `#946418` is 5.13:1, and for the dark-theme
game accent `#7b85e3` the button text should switch to `var(--bg)`
(5.48:1) since white on it is 3.33:1.

The 1 px `--edge` border on buttons is 1.2:1 and is allowed to stay.
1.4.11 does not require a boundary to have contrast when the control is
identifiable without it, and these controls are identified by their icon
and label at 5:1 or better. Do not rely on the edge for anything.

### When an icon may stand alone

One rule, no judgment calls: an icon stands alone only if it is on the
short list below and its action is the obvious one. Everything else gets
a visible text label next to it, in the same element, always.

The short list: close (X), play, pause, search (magnifier), back and
forward arrows, external link, expand and collapse chevrons, menu
(three lines). Even these still need an accessible name via `aria-label`
or visually hidden text.

No icon on the TAN portal today is on the list. The song icons are
jokes; the feature icons are illustrations. Both categories get labels,
and both already have headings or captions doing that job except the
song picker, which is why this document exists.

Corollary: a tooltip never carries the name. If a control needs a tooltip
to explain what it does, that text belongs in a visible label instead. A
tooltip may add detail (the full song title, a keyboard shortcut, a unit)
that the label abbreviates.

### States

Rest: icon and label in `--muted`, 1 px `--edge` border, `--card`
background. 5:1 or better in both themes.

Hover: icon and label in `--ink`, border in `--accent-row`. Hover is a
courtesy for mouse users and carries no meaning, so its contrast is
unconstrained.

Selected: icon and label in `--ink`, label weight 600, 2 px border in
`--accent-row`, 16 percent accent tint on the background. Three cues
(weight, border thickness, tint) so no single one has to do the job.
With the split accents, the 2 px border is at least 3.6:1 against the
card in every row and theme, which satisfies the state clause of 1.4.11.

Focus: `outline: 2px solid var(--ink); outline-offset: 2px`. Ink, not
accent, because ink is 14:1 or better in both themes and two of the four
accents are not. Applies to `:focus-visible` only, so mouse clicks do not
draw a ring.

### The icon + label + tooltip pattern

Requirements, all at once: sighted mouse users see the label and can
hover for the full title; keyboard users tab to the group once, arrow
between options, see the focus ring and the tooltip; screen reader users
hear name, role, checked state, position in group, and the full title as
a description; touch users see the label and never need the tooltip.

Six exclusive options is a radio group. Native `<input type="radio">`
gives role, checked state, one tab stop, arrow-key movement and "3 of 6"
for free, with no ARIA and no keyboard JavaScript. The picker keeps its
look because the input is visually hidden and the `<label>` is styled as
the tile.

Markup for the whole group, with one option shown in full. Replace the
existing `<div class="song-picker" ...>` block with this.

```html
<fieldset class="song-picker" id="song-picker">
  <legend class="sr-only">Choose a song</legend>

  <div class="song-opt">
    <label class="song-btn">
      <input type="radio" name="song" value="dope-peddler" checked
             aria-describedby="tip-dope-peddler" autocomplete="off">
      <svg class="glyph" aria-hidden="true" viewBox="0 0 24 24">
        <path d="M12 3v18M8 7.5c0-1.5 1.5-2.5 4-2.5s4 1 4 2.5-1.5 2.5-4 2.5-4 1-4 2.5 1.5 2.5 4 2.5 4-1 4-2.5"
              fill="none" stroke="currentColor" stroke-width="1.8"
              stroke-linecap="round" stroke-linejoin="round"/>
      </svg>
      <span class="song-label">Dope Peddler</span>
    </label>
    <span class="tip" role="tooltip" id="tip-dope-peddler">The Old Dope Peddler</span>
  </div>

  <!-- five more .song-opt blocks, same shape:
       fight-fiercely  "Fight Fiercely"  tip "Fight Fiercely, Harvard"
       hunting-song    "Hunting Song"    tip "The Hunting Song"
       be-prepared     "Be Prepared"     tip "Be Prepared"
       lobachevsky     "Lobachevsky"     tip "Lobachevsky"
       hold-your-hand  "Hold Your Hand"  tip "I Hold Your Hand in Mine" -->
</fieldset>
```

Why each piece is there. The `<legend>` names the group so a screen
reader says "Choose a song" on entry; it is visually hidden because the
paragraph above already says it. The tooltip sits outside the `<label>`
so its text does not get folded into the radio's name (a label's name is
its full text content); `aria-describedby` on the input attaches it as a
description instead, and descriptions may reference hidden content, so
screen readers hear it even while it is visually hidden. `aria-hidden`
on the SVG keeps "graphic" out of the announcement. `autocomplete="off"`
stops the browser restoring a previously checked radio on reload while
the audio `src` still points at the default.

Where "Be Prepared" and "Lobachevsky" have a tooltip identical to the
label, drop the tooltip and the `aria-describedby` on those two. A
tooltip that repeats the label is noise.

CSS. Replace the current `.song-picker`, `.song-btn`, `.song-btn svg`,
`.song-btn .song-label`, `:hover`, `:focus-visible` and `.active` rules.
Everything below works in any browser that supports `color-mix()`, which
the page already requires.

```css
.sr-only {
  position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px;
  overflow: hidden; clip: rect(0 0 0 0); white-space: nowrap; border: 0;
}

.song-picker {
  display: flex; flex-wrap: wrap; gap: 0.6rem;
  margin: 0.75rem 0 1.1rem; padding: 0; border: 0; min-width: 0;
}
.song-opt { position: relative; }

.song-btn {
  display: flex; flex-direction: column; align-items: center;
  justify-content: center; gap: 0.3rem;
  width: 76px; min-height: 64px;            /* > 44x44, SC 2.5.5 */
  padding: 0.5rem 0.35rem;
  background: var(--card);
  border: 1px solid var(--edge);
  border-radius: 10px;
  color: var(--muted);                      /* 5.0:1 light, 6.2:1 dark */
  cursor: pointer; font: inherit;
}
.song-btn input { position: absolute; width: 1px; height: 1px; margin: -1px;
                  overflow: hidden; clip: rect(0 0 0 0); }
.song-btn .glyph { width: 22px; height: 22px; flex: none; }
.song-btn .song-label { font-size: 0.68rem; line-height: 1.15; text-align: center; }

.song-btn:hover { color: var(--ink); border-color: var(--accent-row, var(--accent)); }

.song-btn:has(input:checked) {
  color: var(--ink);
  border: 2px solid var(--accent-row, var(--accent));
  padding: calc(0.5rem - 1px) calc(0.35rem - 1px);   /* keep the tile the same size */
  background: color-mix(in srgb, var(--accent-row, var(--accent)) 16%, var(--card));
}
.song-btn:has(input:checked) .song-label { font-weight: 600; }

.song-btn:has(input:focus-visible) { outline: 2px solid var(--ink); outline-offset: 2px; }

/* Tooltip: hoverable (no gap, bridged by ::before), persistent while
   hovered or focused, dismissible via Escape (data-tips-hidden). */
.tip {
  position: absolute; left: 50%; bottom: 100%; transform: translateX(-50%);
  margin-bottom: 6px; padding: 0.3rem 0.55rem;
  background: var(--ink); color: var(--bg);
  font-size: 0.75rem; line-height: 1.3; white-space: nowrap; border-radius: 6px;
  visibility: hidden; opacity: 0; transition: opacity 0.12s;
  z-index: 2; pointer-events: auto;
}
.tip::before { content: ""; position: absolute; left: 0; right: 0; top: 100%; height: 8px; }
.song-opt:hover .tip,
.song-opt:has(input:focus-visible) .tip { visibility: visible; opacity: 1; }
.song-picker[data-tips-hidden] .tip { visibility: hidden; opacity: 0; }
@media (prefers-reduced-motion: reduce) { .tip { transition: none; } }
```

JavaScript. The click handler that toggles `.active` goes away; the
browser owns the checked state. Replace the `.song-btn` click block with
a `change` listener on the fieldset and the three lines for Escape.

```js
const picker = document.getElementById("song-picker");

function selectSong(key) {
  origMusic.pause();
  origMusic.src = `audio/samples/music-lehrer-${key}.mp3`;
  musicTitle.textContent = songTitles[key];
  // existing stale-result cleanup stays here unchanged
}

picker.addEventListener("change", (e) => selectSong(e.target.value));
selectSong(picker.querySelector('input[name="song"]:checked').value);

picker.addEventListener("keydown", (e) => {
  if (e.key === "Escape") picker.setAttribute("data-tips-hidden", "");
});
picker.addEventListener("pointermove", () => picker.removeAttribute("data-tips-hidden"));
picker.addEventListener("focusin", () => picker.removeAttribute("data-tips-hidden"));
```

What each audience now gets. Mouse: label always visible, full title on
hover, tooltip stays while the pointer moves onto it. Keyboard: Tab lands
on the checked song, arrow keys move and select, ink ring shows where
focus is, tooltip appears on focus and Escape hides it without moving
focus. Screen reader: "Choose a song, group. Dope Peddler, radio button,
checked, 1 of 6, The Old Dope Peddler." Touch: label, tile, done. Voice
control: "click Dope Peddler" matches the visible text because the
visible text is the name.

### Checklist for any new icon

- Which family, glyph (24-grid, 1.8) or art (100-grid 4 or 200-grid 3)?
- `currentColor` for glyphs; literal accent hex only for art.
- `aria-hidden="true"` on every decorative SVG, which is all of them.
- Visible text label in the same element unless the icon is on the short
  list.
- Whatever sets `color` on the parent passes 4.5:1 on both themes.
- Any accent used for a state passes 3:1 on both themes (table above).
- Clickable: at least 44 by 44 CSS px, or 24 by 24 with clear spacing.
- Selected state has a non-hue cue (weight, border width, shape).
- Focus ring in `--ink`, on `:focus-visible` only.
- Tooltips carry extras, never the name; hoverable, persistent, Escape
  to dismiss.
