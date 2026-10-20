import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

/** jsdom no implementa los observadores que usan los paneles. */
class NoopObserver {
  observe(): void {}
  unobserve(): void {}
  disconnect(): void {}
}
vi.stubGlobal("ResizeObserver", NoopObserver);
vi.stubGlobal("IntersectionObserver", NoopObserver);

const invokeMocks = new Map<string, (args: unknown) => unknown>();

export function setInvokeMock(
  command: string,
  handler: (args: unknown) => unknown,
): void {
  invokeMocks.set(command, handler);
}

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn((command: string, args?: unknown) => {
    const handler = invokeMocks.get(command);
    if (!handler) {
      return Promise.reject(new Error(`[test] invoke("${command}") sin mock registrado`));
    }
    try {
      return Promise.resolve(handler(args));
    } catch (error) {
      return Promise.reject(error);
    }
  }),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    onDragDropEvent: vi.fn().mockResolvedValue(() => {}),
    onResized: vi.fn().mockResolvedValue(() => {}),
    onCloseRequested: vi.fn().mockResolvedValue(() => {}),
    isFullscreen: vi.fn().mockResolvedValue(false),
    destroy: vi.fn().mockResolvedValue(undefined),
  }),
}));
