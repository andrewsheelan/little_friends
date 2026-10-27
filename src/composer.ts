export const WAITING_STATUS = new Set([
  "Ollama is unavailable.",
  "No chat model is installed.",
]);

export function isWaiting(status: string): boolean {
  return WAITING_STATUS.has(status);
}

/** Decide whether Enter sends, and what the box should show right away. */
export function planSubmit(text: string, status: string): { send: boolean; box: string } {
  if (text.trim() === "" || isWaiting(status)) {
    return { send: false, box: text };
  }
  return { send: true, box: "" };
}

/** Listen starts a hands-free conversation. Stop stays available until it ends. */
export function listenControl(
  conversing: boolean,
  earReady: boolean,
  micAllowed: boolean,
): { label: string; disabled: boolean } {
  if (conversing) {
    return { label: "Stop", disabled: false };
  }
  return { label: "Listen", disabled: !earReady || !micAllowed };
}

/** Put a rejected line back unless the user has already typed something else. */
export function afterRejectedSend(box: string, sent: string, status: string): string {
  if (isWaiting(status) && box === "") {
    return sent;
  }
  return box;
}
