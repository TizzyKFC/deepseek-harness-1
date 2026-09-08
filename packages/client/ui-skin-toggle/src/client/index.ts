/**
 * Blue Fantasy skin toggle, browser half: a sidebar row that switches between
 * the built-in Blue Fantasy skin and the default dsh theme. The choice is a
 * durable dsh setting (`skin.theme`), so the toggle works entirely inside the
 * harness UI — no desktop-shell bridge needed — and applies instantly via the
 * settings hot-reload channel, persisting across restarts.
 *
 * This plugin owns the Blue Fantasy skin itself (styles, whale backdrop, and
 * favicon are vendored in `blue-fantasy-assets.ts`, adapted from the upstream
 * DreamSkin「DeepSeek-鲸鱼娘」package), so it is fully self-contained: the
 * upstream skin package is not loaded at runtime.
 *
 * The row is plain DOM injected into the sidebar (the task-board precedent:
 * the sidebar shell exposes no external registration slot, so family plugins
 * mount at the DOM level). A MutationObserver self-heals the row across React
 * re-renders; the injected element never participates in the shell's
 * reconciliation.
 */
import type {
  ClientContext, SettingsScope, SettingsScopeSnapshot,
} from '@deepseek-ai/dsh-client-runtime/client'
// Type-only: pulls the `settingsScope` Context merge from the settings surface.
import type {} from '@deepseek-ai/dsh-client-ui-settings/client'
import {
  BLUE_FANTASY_CSS, SCRIM_DARK, SCRIM_LIGHT, WHALE_ART, WHALE_ICON,
} from './blue-fantasy-assets.ts'

/** Settings namespace and field owned by this plugin. */
const NS = 'skin'
const THEME_FIELD = 'theme'

type ThemeKey = 'blue-fantasy' | 'default'

/** The durable skin setting shape. */
interface SkinSettings {
  theme?: ThemeKey
}

/** Required services (cordis fiber inject): the settings-namespace scope. */
export const inject = ['settingsScope']

/** The sidebar shell root, or undefined while the shell is not yet mounted. */
function sidebarRoot(): HTMLElement | undefined {
  const column = document.querySelector<HTMLElement>('[data-pane="sidebar"], [class*="sidebarCol"]')
  if (column === null) return undefined
  const logoOwner = column.querySelector<HTMLElement>('[class*="logoRow"]')?.parentElement
  return logoOwner ?? (column.firstElementChild as HTMLElement | undefined)
}

/** The New Session button (current and legacy shell layouts). */
function newSessionButton(root: HTMLElement): HTMLButtonElement | undefined {
  const nested = root.querySelector<HTMLButtonElement>('button[class*="newSession"]')
  if (nested !== null) return nested
  for (const child of root.children) {
    if (child instanceof HTMLButtonElement) return child
  }
  return undefined
}

/** The <style> tag this plugin owns for the Blue Fantasy stylesheet. */
const STYLE_SELECTOR = 'style[data-dsh-skin-toggle]'
/** The <link rel="icon"> tag this plugin owns while Blue Fantasy is applied. */
const FAVICON_SELECTOR = 'link[data-dsh-skin-toggle-favicon]'

/** Apply the Blue Fantasy skin: body attribute, stylesheet, whale backdrop, favicon. */
function applyBlueFantasy(): void {
  const body = document.body
  body.dataset.dshBlueFantasy = ''
  if (document.querySelector(STYLE_SELECTOR) === null) {
    const tag = document.createElement('style')
    tag.setAttribute('data-dsh-skin-toggle', '')
    tag.textContent = BLUE_FANTASY_CSS
    document.head.appendChild(tag)
  }
  const dark = body.dataset.dsDarkTheme !== undefined
  const scrim = (dark ? SCRIM_DARK : SCRIM_LIGHT).join(', ')
  body.style.backgroundImage = `${scrim}, url(${WHALE_ART})`
  body.style.backgroundPosition = 'center'
  body.style.backgroundSize = 'cover'
  body.style.backgroundAttachment = 'fixed'
  body.style.backgroundRepeat = 'no-repeat'
  if (document.querySelector(FAVICON_SELECTOR) === null) {
    const link = document.createElement('link')
    link.rel = 'icon'
    link.type = 'image/png'
    link.dataset.dshSkinToggleFavicon = ''
    link.href = WHALE_ICON
    document.head.appendChild(link)
  }
}

/** Restore the default dsh theme (remove everything this plugin applied). */
function applyDefault(): void {
  const body = document.body
  delete body.dataset.dshBlueFantasy
  body.style.backgroundImage = ''
  body.style.backgroundPosition = ''
  body.style.backgroundSize = ''
  body.style.backgroundAttachment = ''
  body.style.backgroundRepeat = ''
  document.querySelector(STYLE_SELECTOR)?.remove()
  document.querySelector(FAVICON_SELECTOR)?.remove()
}

/** Resolve the theme key from a settings snapshot; Blue Fantasy is the default. */
function themeOf(snapshot: SettingsScopeSnapshot<SkinSettings>): ThemeKey {
  return snapshot.status === 'ready' && snapshot.value?.theme === 'default' ? 'default' : 'blue-fantasy'
}

/** A 14px palette glyph matching the shell's nav-icon look. */
function icon(): SVGSVGElement {
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg')
  svg.setAttribute('viewBox', '0 0 16 16')
  svg.setAttribute('width', '14')
  svg.setAttribute('height', '14')
  svg.setAttribute('fill', 'none')
  svg.setAttribute('stroke', 'currentColor')
  svg.setAttribute('stroke-width', '1.3')
  svg.setAttribute('stroke-linecap', 'round')
  svg.setAttribute('stroke-linejoin', 'round')
  svg.setAttribute('aria-hidden', 'true')
  svg.innerHTML = '<circle cx="5" cy="5" r="2.2"/><circle cx="11" cy="5" r="2.2"/><circle cx="5" cy="11" r="2.2"/><circle cx="11" cy="11" r="2.2"/>'
  return svg
}

/** Sync the applied skin and the row label with the current settings value. */
function syncSkin(scope: SettingsScope<SkinSettings>, label: HTMLSpanElement): void {
  const theme = themeOf(scope.getSnapshot())
  if (theme === 'blue-fantasy') applyBlueFantasy()
  else applyDefault()
  label.textContent = theme === 'blue-fantasy' ? '主题：Blue Fantasy' : '主题：默认'
}

/** Inject the toggle row's layout stylesheet once. Mirrors the task-board
 * entry: a full-width row whose label collapses away when the sidebar is
 * collapsed, leaving the icon centered. Kept separate from the theme styles
 * (`data-dsh-skin-toggle`) because it must apply in both themes. */
function injectLayoutStyles(): void {
  if (document.querySelector('style[data-dsh-skin-toggle-layout]') !== null) return
  const tag = document.createElement('style')
  tag.setAttribute('data-dsh-skin-toggle-layout', '')
  tag.textContent = [
    '.dsh-skin-toggle-entry{width:100%;height:32px;color:var(--dsw-alias-label-secondary);cursor:pointer;white-space:nowrap;background:transparent;border:0;border-radius:8px;align-items:center;gap:8px;padding:0 12px;margin:2px 0;font:inherit;font-size:13px;text-align:left;display:flex}',
    '.dsh-skin-toggle-entry:hover{background:var(--dsw-specific-sidebar-nav-item-hover,var(--dsw-alias-bg-hover,rgba(128,128,128,.12)));color:var(--dsw-alias-label-primary,currentColor)}',
    '.dsh-skin-toggle-label{white-space:nowrap;text-overflow:ellipsis;overflow:hidden}',
    '[data-sidebar-collapsed] .dsh-skin-toggle-entry{justify-content:center;width:100%;padding:0}',
    '[data-sidebar-collapsed] .dsh-skin-toggle-label{display:none}',
  ].join('')
  document.head.appendChild(tag)
}

/** Build the detached toggle row (inserted once the sidebar is up). */
function createEntry(scope: SettingsScope<SkinSettings>): HTMLButtonElement {
  const entry = document.createElement('button')
  entry.type = 'button'
  entry.dataset.dshSkinToggle = ''
  entry.setAttribute('aria-label', '切换主题')
  entry.className = 'dsh-skin-toggle-entry'
  entry.addEventListener('click', () => {
    const theme = themeOf(scope.getSnapshot())
    void scope.set(THEME_FIELD, theme === 'blue-fantasy' ? 'default' : 'blue-fantasy')
  })
  entry.appendChild(icon())
  const label = document.createElement('span')
  label.className = 'dsh-skin-toggle-label'
  entry.appendChild(label)
  return entry
}

/** Insert the row next to the New Session block (the family anchor). */
function placeEntry(root: HTMLElement, entry: HTMLButtonElement): boolean {
  const button = newSessionButton(root)
  if (button === undefined) return false
  if (entry.parentElement !== root) {
    const row = button.closest('[class*="logoRow"]')
    const base = (row !== null && row.parentElement === root) ? row : button
    root.insertBefore(entry, base.nextElementSibling)
  }
  return true
}

/**
 * Mount the toggle row, waiting for the shell to render and self-healing on
 * later React re-renders.
 * @param entry - the detached toggle row.
 * @returns disposer removing the row and its observers.
 */
function mountEntry(entry: HTMLButtonElement): () => void {
  let root: HTMLElement | undefined
  let placed = false
  const rootObserver = new MutationObserver(() => { tryPlace() })
  const tryPlace = (): void => {
    if (root !== undefined && !root.isConnected) {
      rootObserver.disconnect()
      root = undefined
      placed = false
    }
    if (placed) {
      if (document.body.contains(entry)) return
      rootObserver.disconnect()
      root = undefined
      placed = false
    }
    root ??= sidebarRoot()
    if (root === undefined) return
    placed = placeEntry(root, entry)
    if (placed) rootObserver.observe(root, { childList: true, subtree: true })
  }
  const watcher = new MutationObserver(tryPlace)
  watcher.observe(document.body, { childList: true, subtree: true })
  tryPlace()
  return () => {
    watcher.disconnect()
    rootObserver.disconnect()
    entry.remove()
  }
}

/**
 * Client plugin body: read the durable skin setting, mount the sidebar toggle,
 * and keep the applied theme in sync with the setting (instant, hot-reloaded).
 * @param ctx - client root context.
 */
export function apply(ctx: ClientContext): void {
  injectLayoutStyles()
  const scope = ctx.settingsScope.bind<SkinSettings>({ namespace: NS })
  const entry = createEntry(scope)
  const label = entry.querySelector('span') as HTMLSpanElement
  const mountDispose = mountEntry(entry)
  const subDispose = scope.subscribe(() => { syncSkin(scope, label) })
  // Apply the initial theme; the settings subscription then corrects it once
  // the durable value arrives (Blue Fantasy is the default).
  syncSkin(scope, label)
  ctx.effect(() => () => {
    subDispose()
    mountDispose()
    applyDefault()
  }, 'ui-skin-toggle: cleanup')
}
