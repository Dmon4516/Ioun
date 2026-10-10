let timer: ReturnType<typeof setTimeout> | undefined;

export const toast = $state({
  message: "",
  visible: false,
});

export function showToast(message: string, duration = 2200): void {
  toast.message = message;
  toast.visible = true;
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => {
    toast.visible = false;
    timer = undefined;
  }, duration);
}

export function hideToast(): void {
  if (timer) clearTimeout(timer);
  timer = undefined;
  toast.visible = false;
}
