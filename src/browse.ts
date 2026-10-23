import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';
import { usePad } from './pad';

// Choosing files and folders. With mouse and keyboard the system's own windows open; while a
// gamepad drives the app (they can't be used with one, and on the Steam Deck's Game Mode they
// leave the app) the in-app browser (components/FileBrowser.tsx) opens instead.

export type BrowseMode = 'file' | 'folder' | 'save';

export interface BrowseOptions {
  mode: BrowseMode;
  title?: string;
  /** Extensions to offer, without the dot, e.g. ['sl2']. */
  extensions?: string[];
  filterName?: string;
  /** A folder to start in, or for 'save' the suggested file. */
  defaultPath?: string;
}

interface Request extends BrowseOptions {
  resolve: (path: string | null) => void;
}

export const useBrowser = create<{ request: Request | null }>(() => ({ request: null }));

/** A path chosen by the user, or null when they cancel. */
export async function choosePath(o: BrowseOptions): Promise<string | null> {
  if (usePad.getState().active) {
    return new Promise((resolve) => useBrowser.setState({ request: { ...o, resolve } }));
  }
  const filters = o.extensions ? [{ name: o.filterName ?? o.extensions.join(', '), extensions: o.extensions }] : undefined;
  if (o.mode === 'save') return (await save({ defaultPath: o.defaultPath, filters, title: o.title })) ?? null;
  const picked = await open({ directory: o.mode === 'folder', multiple: false, filters, title: o.title, defaultPath: o.defaultPath });
  return typeof picked === 'string' ? picked : null;
}

export interface Place {
  path: string;
  kind: 'saves' | 'home' | 'drive' | 'media';
}

export interface DirEntry {
  name: string;
  path: string;
  dir: boolean;
  size: number;
  modified: number;
}

export interface Listing {
  path: string;
  parent: string | null;
  entries: DirEntry[];
}

export const filesApi = {
  places: () => invoke<Place[]>('browse_places'),
  list: (path: string) => invoke<Listing>('list_dir', { path })
};
