import { describe, expect, it, vi } from "vitest";
import { handleCloseRequested } from "./closeHandler";

describe("handleCloseRequested", () => {
  it("does not flush or intercept when flushOnClose is disabled", async () => {
    const event = { preventDefault: vi.fn() };
    const flushSettings = vi.fn().mockResolvedValue(undefined);
    const flushPendingProgress = vi.fn().mockResolvedValue(undefined);
    const destroy = vi.fn().mockResolvedValue(undefined);

    await handleCloseRequested(
      event,
      false,
      flushSettings,
      flushPendingProgress,
      destroy,
    );

    expect(event.preventDefault).not.toHaveBeenCalled();
    expect(flushSettings).not.toHaveBeenCalled();
    expect(flushPendingProgress).not.toHaveBeenCalled();
    expect(destroy).not.toHaveBeenCalled();
  });

  it("intercepts, flushes both stores, and destroys when enabled", async () => {
    const event = { preventDefault: vi.fn() };
    const flushSettings = vi.fn().mockResolvedValue(undefined);
    const flushPendingProgress = vi.fn().mockResolvedValue(undefined);
    const destroy = vi.fn().mockResolvedValue(undefined);

    await handleCloseRequested(
      event,
      true,
      flushSettings,
      flushPendingProgress,
      destroy,
    );

    expect(event.preventDefault).toHaveBeenCalledOnce();
    expect(flushSettings).toHaveBeenCalledOnce();
    expect(flushPendingProgress).toHaveBeenCalledOnce();
    expect(destroy).toHaveBeenCalledOnce();
  });

  it("reports a flush error and still destroys the window", async () => {
    const event = { preventDefault: vi.fn() };
    const error = new Error("flush failed");
    const reportError = vi.fn();
    const flushSettings = vi.fn().mockRejectedValue(error);
    const flushPendingProgress = vi.fn().mockResolvedValue(undefined);
    const destroy = vi.fn().mockResolvedValue(undefined);

    await handleCloseRequested(
      event,
      true,
      flushSettings,
      flushPendingProgress,
      destroy,
      reportError,
    );

    expect(reportError).toHaveBeenCalledWith(error);
    expect(destroy).toHaveBeenCalledOnce();
  });
});
