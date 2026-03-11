import { writable, derived, type Readable } from 'svelte/store';
import {
  DEFAULT_DESKTOP_STATE,
  type DesktopState,
  type DesktopView,
  type BackendStatusPayload,
  type UpdateStatusPayload,
  type GameSummary,
  type Game,
  type LegalMove,
  type ArchivedGameSummary,
  type StorageStats,
  type ReplayState,
  type AnalysisJob,
} from './shared-types.js';

export type ConfirmModalState = {
  kind: 'confirm';
  title: string;
  message: string;
  confirmLabel: string;
  cancelLabel: string;
  resolve: (value: boolean) => void;
};

export type PromptModalState = {
  kind: 'prompt';
  title: string;
  message: string;
  confirmLabel: string;
  cancelLabel: string;
  initialValue: string;
  placeholder?: string;
  resolve: (value: string | null) => void;
};

export type ModalState = ConfirmModalState | PromptModalState;

// Desktop state stores
export const desktopState = writable<DesktopState>({ ...DEFAULT_DESKTOP_STATE });
export const currentView = writable<DesktopView>('dashboard');
export const backendStatus = writable<BackendStatusPayload>({
  running: false,
  pid: null,
  command: null,
  startedAt: null,
  exitCode: null,
  lastError: null,
});
export const backendLogs = writable('');
export const updateStatus = writable<UpdateStatusPayload>({
  supported: false,
  currentVersion: 'dev',
  state: 'unsupported',
  availableVersion: null,
  percent: null,
  transferredBytes: null,
  totalBytes: null,
  message: null,
});

// UI state stores
export const paletteOpen = writable(false);
export const paletteQuery = writable('');
export const toastMsg = writable<string | null>(null);
export const errorMsg = writable<string | null>(null);
export const modalState = writable<ModalState | null>(null);
export const boardAscii = writable('');
export const liveConnection = writable<'connecting' | 'connected' | 'disconnected'>(
  'disconnected'
);
export const liveMessage = writable('Live sync offline');
export const importDropActive = writable(false);

// Engine data stores
export const gamesList = writable<GameSummary[]>([]);
export const activeGame = writable<Game | null>(null);
export const legalMoves = writable<LegalMove[]>([]);
export const selectedSquare = writable<string | null>(null);
export const archivedList = writable<ArchivedGameSummary[]>([]);
export const storageStats = writable<StorageStats | null>(null);
export const replayState = writable<ReplayState | null>(null);
export const replayGameId = writable<string | null>(null);
export const analysisJobs = writable<AnalysisJob[]>([]);
export const activeAnalysis = writable<AnalysisJob | null>(null);
export const fenInput = writable('');
export const analysisDepth = writable(30);
export const analysisPolling = writable(false);

// Computed stores (derived)
export const boardFlipped: Readable<boolean> = derived(
  desktopState,
  ($desktopState) => $desktopState.boardFlipped
);

export const highlightSquares: Readable<Set<string>> = derived(
  [selectedSquare, legalMoves],
  ([$selectedSquare, $legalMoves]): Set<string> => {
    if (!$selectedSquare) return new Set<string>();
    return new Set<string>(
      $legalMoves.filter((m) => m.from === $selectedSquare).map((m) => m.to)
    );
  }
);

export const lastMove: Readable<{ from: string; to: string } | null> = derived(
  activeGame,
  ($activeGame) => {
    if (!$activeGame || $activeGame.move_history.length === 0) return null;
    const last = $activeGame.move_history[$activeGame.move_history.length - 1];
    return {
      from: last.move_json.from,
      to: last.move_json.to,
    };
  }
);
