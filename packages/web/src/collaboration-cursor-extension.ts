import { Extension } from '@tiptap/core';
import { yCursorPlugin } from '@tiptap/y-tiptap';
import { cursorBuilder } from './cursor-renderer';
import type { WebsocketProvider } from 'y-websocket';

type Awareness = WebsocketProvider['awareness'];

export interface CollaborationCursorOptions {
  awareness: Awareness;
  user: { name: string; color: string };
}

export const CollaborationCursor = Extension.create<CollaborationCursorOptions>({
  name: 'collaborationCursor',

  addOptions() {
    return {
      awareness: null as unknown as Awareness,
      user: { name: 'Anonymous', color: '#aaaaaa' },
    };
  },

  addProseMirrorPlugins() {
    const { awareness, user } = this.options;

    awareness.setLocalStateField('user', user);

    return [yCursorPlugin(awareness, { cursorBuilder })];
  },
});
