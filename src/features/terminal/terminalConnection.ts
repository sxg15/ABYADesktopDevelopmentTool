import type { TerminalProvider } from "../../shared/types";

// A UI attachment request, scoped to one existing conversation. Never creates a chat.
export interface TerminalConnectionRequest {
  id: string;
  taskId: string;
  provider: TerminalProvider;
  conversationId: string;
  finish: (error?: unknown) => void;
}
