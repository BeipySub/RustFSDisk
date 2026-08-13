/// <reference types="vite/client" />

import type { VNode } from "vue";

declare global {
  namespace JSX {
    type Element = VNode;

    interface IntrinsicElements {
      [element: string]: unknown;
    }
  }
}

export {};
