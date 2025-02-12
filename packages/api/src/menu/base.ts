// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0, MIT

import { Channel, invoke, Resource } from '../core'
import { transformImage } from '../image'
import { CheckMenuItemOptions, IconMenuItemOptions, MenuOptions, MenuItemOptions, PredefinedMenuItemOptions, SubmenuOptions } from './types'

export type ItemKind = 'MenuItem' | 'Predefined' | 'Check' | 'Icon' | 'Submenu' | 'Menu'
type MenuItemOptionsAlias = MenuItemOptions | SubmenuOptions | IconMenuItemOptions | PredefinedMenuItemOptions | CheckMenuItemOptions

function injectChannel(i: MenuItemOptionsAlias): MenuItemOptionsAlias & { handler?: Channel<string> } {
  if ('items' in i) i.items = i.items?.map(item => ('rid' in item ? item : injectChannel(item)))
  if ('action' in i) {
    const handler = new Channel<string>()
    handler.onmessage = i.action
    delete i.action
    return { ...i, handler }
  }
  return i
}

export async function newMenu(kind: ItemKind, opts?: MenuOptions | MenuItemOptionsAlias): Promise<[number, string]> {
  const handler = new Channel<string>()

  if (opts && typeof opts === 'object') {
    if ('action' in opts) {
      handler.onmessage = opts.action as () => void
      delete opts.action
    }

    const processIcon = (obj: any) => obj?.icon && (obj.icon = transformImage(obj.icon))

    processIcon(opts)
    if ('item' in opts) processIcon(opts.item?.About)
    if ('items' in opts) opts.items = opts.items.map(item => ('rid' in item ? [item.rid, item.kind] : injectChannel(item)))

  }

  return invoke('plugin:menu|new', { kind, options: opts, handler })
}

export class MenuItemBase extends Resource {
  /** The id of this item. */
  get id(): string { return this.#id }
  /** @ignore */
  get kind(): string { return this.#kind }

  /** @ignore */
  protected constructor(rid: number, private readonly #id: string, private readonly #kind: ItemKind) {
    super(rid)
  }
}
