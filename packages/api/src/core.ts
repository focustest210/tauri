// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0, MIT

import { Channel, invoke } from '../core'

export const SERIALIZE_TO_IPC_FN = '__TAURI_TO_IPC_KEY__'

function transformCallback<T = unknown>(
  callback?: (response: T) => void,
  once = false
): number {
  return window.__TAURI_INTERNALS__.transformCallback(callback, once)
}

class PluginListener {
  constructor(
    public plugin: string,
    public event: string,
    public channelId: number
  ) {}

  async unregister(): Promise<void> {
    return invoke(`plugin:${this.plugin}|remove_listener`, {
      event: this.event,
      channelId: this.channelId
    })
  }
}

async function addPluginListener<T>(
  plugin: string,
  event: string,
  cb: (payload: T) => void
): Promise<PluginListener> {
  const handler = new Channel<T>()
  handler.onmessage = cb
  await invoke(`plugin:${plugin}|registerListener`, { event, handler })
  return new PluginListener(plugin, event, handler.id)
}

async function checkPermissions<T>(plugin: string): Promise<T> {
  return invoke(`plugin:${plugin}|check_permissions`)
}

async function requestPermissions<T>(plugin: string): Promise<T> {
  return invoke(`plugin:${plugin}|request_permissions`)
}

async function invoke<T>(
  cmd: string,
  args: Record<string, unknown> = {},
  options?: { headers: Headers | Record<string, string> }
): Promise<T> {
  return window.__TAURI_INTERNALS__.invoke(cmd, args, options)
}

function convertFileSrc(filePath: string, protocol = 'asset'): string {
  return window.__TAURI_INTERNALS__.convertFileSrc(filePath, protocol)
}

class Resource {
  constructor(private readonly #rid: number) {}

  get rid(): number {
    return this.#rid
  }

  async close(): Promise<void> {
    return invoke('plugin:resources|close', { rid: this.#rid })
  }
}

function isTauri(): boolean {
  return Boolean(window.isTauri)
}

export type InvokeArgs = Record<string, unknown>
export type InvokeOptions = { headers: Headers | Record<string, string> }

export {
  transformCallback,
  Channel,
  PluginListener,
  addPluginListener,
  checkPermissions,
  requestPermissions,
  invoke,
  convertFileSrc,
  isTauri,
  Resource
}
