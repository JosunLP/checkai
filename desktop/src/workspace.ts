import { get } from 'svelte/store';
import {
  cancelAnalysisJob,
  createGame,
  deleteGame,
  exportFen,
  exportPgn,
  getAnalysisJob,
  getBoardAscii,
  getGame,
  getLegalMoves,
  importFen,
  listAnalysisJobs,
  listArchived,
  listGames,
  replayArchived,
  setApiBase,
  startAnalysis,
  submitMove,
} from './api-client.js';
import { desktop, loadDesktopState, saveDesktopState } from './desktop-api.js';
import {
  clearNotificationTimers,
  pushError,
  pushToast,
} from './notifications.js';
import { attemptRefresh, refreshStore } from './workspace/refresh.js';
import {
  activeAnalysis,
  activeGame,
  analysisDepth,
  analysisJobs,
  archivedList,
  backendLogs,
  backendStatus,
  boardAscii,
  currentView,
  desktopState,
  fenInput,
  gamesList,
  legalMoves,
  modalState,
  paletteOpen,
  replayGameId,
  replayState,
  selectedSquare,
  storageStats,
  updateStatus,
} from './stores.js';
import type {
  AnalysisJob,
  AnalysisStatus,
  BackendPreset,
  DesktopState,
  DesktopView,
  SaveTextFileOptions,
} from './shared-types.js';

const ANALYSIS_POLL_INTERVAL_MS = 2000;
const WORKSPACE_REFRESH_INTERVAL_MS = 5000;
const MAX_WORKSPACE_REFRESH_BACKOFF_MULTIPLIER = 6;
const ID_DISPLAY_LENGTH = 8;
const MIN_ANALYSIS_DEPTH = 30;
const MAX_ANALYSIS_DEPTH = 99;
const systemThemeMedia =
  typeof window !== 'undefined' ? window.matchMedia('(prefers-color-scheme: light)') : null;

let analysisPollTimer: ReturnType<typeof setInterval> | null = null;
let analysisRefreshInFlight = false;
let analysisPollingSession = 0;
let workspaceRefreshTimer: ReturnType<typeof setInterval> | null = null;
let workspaceRefreshInFlight = false;
let workspacePollingBackoffUntil = 0;
let workspacePollingFailureCount = 0;
let menuCommandCleanup: (() => void) | null = null;

function slugify(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '') || 'checkai-game';
}

export function isInProgressStatus(
  status: AnalysisStatus
): status is { InProgress: { moves_analyzed: number; total_moves: number } } {
  return typeof status === 'object' && status !== null && 'InProgress' in status;
}

export function isFailedStatus(
  status: AnalysisStatus
): status is { Failed: { error: string } } {
  return typeof status === 'object' && status !== null && 'Failed' in status;
}

export function isTerminalAnalysisStatus(status: AnalysisStatus): boolean {
  return status === 'Completed' || status === 'Cancelled' || isFailedStatus(status);
}

async function notifyIfEnabled(title: string, body: string): Promise<void> {
  if (!get(desktopState).notificationsEnabled) {
    return;
  }

  try {
    await desktop.notify(title, body);
  } catch {
    // Ignore notification delivery failures in the renderer.
  }
}

function shortId(value: string): string {
  return value.slice(0, ID_DISPLAY_LENGTH);
}

async function confirmAction(options: {
  title: string;
  message: string;
  confirmLabel: string;
  cancelLabel?: string;
}): Promise<boolean> {
  return new Promise((resolve) => {
    modalState.set({
      kind: 'confirm',
      title: options.title,
      message: options.message,
      confirmLabel: options.confirmLabel,
      cancelLabel: options.cancelLabel ?? 'Cancel',
      resolve,
    });
  });
}

async function promptForValue(options: {
  title: string;
  message: string;
  confirmLabel: string;
  cancelLabel?: string;
  initialValue?: string;
  placeholder?: string;
}): Promise<string | null> {
  return new Promise((resolve) => {
    modalState.set({
      kind: 'prompt',
      title: options.title,
      message: options.message,
      confirmLabel: options.confirmLabel,
      cancelLabel: options.cancelLabel ?? 'Cancel',
      initialValue: options.initialValue ?? '',
      placeholder: options.placeholder,
      resolve,
    });
  });
}

async function copyText(value: string, successMessage: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(value);
    pushToast(successMessage);
  } catch (error) {
    pushError(error instanceof Error ? error.message : String(error));
  }
}

async function saveTextExport(
  defaultPath: string,
  content: string,
  filters: SaveTextFileOptions['filters'],
  successMessage: string
): Promise<void> {
  try {
    const savedPath = await desktop.saveTextFile({
      defaultPath,
      content,
      filters,
    });

    if (!savedPath) {
      return;
    }

    pushToast(successMessage);
    await notifyIfEnabled('CheckAI Desktop', `${successMessage} (${savedPath})`);
  } catch (error) {
    pushError(error instanceof Error ? error.message : String(error));
  }
}

function applyDesktopState(nextState: DesktopState, persist = true): void {
  desktopState.set(nextState);
  setApiBase(nextState.backendUrl);
  syncDocumentTheme(nextState.theme);
  if (persist) {
    void saveDesktopState();
  }
}

function resolveTheme(theme: DesktopState['theme']): 'dark' | 'light' {
  if (theme === 'system') {
    return systemThemeMedia?.matches ? 'light' : 'dark';
  }

  return theme;
}

function syncDocumentTheme(theme: DesktopState['theme']): void {
  document.documentElement.setAttribute('data-theme', resolveTheme(theme));
}

export function updateDesktopState(
  updater: (state: DesktopState) => DesktopState,
  persist = true
): DesktopState {
  const nextState = updater(get(desktopState));
  applyDesktopState(nextState, persist);
  return nextState;
}

export function navigateTo(view: DesktopView): void {
  currentView.set(view);
  updateDesktopState((state) => ({ ...state, lastView: view }));
}

async function refreshBackendState(silent = false): Promise<void> {
  await refreshStore(silent, () => desktop.getBackendStatus(), (status) => {
    backendStatus.set(status);
  });
}

async function refreshUpdateState(silent = false): Promise<void> {
  await refreshStore(silent, () => desktop.getUpdateStatus(), (status) => {
    updateStatus.set(status);
  });
}

export async function refreshLogs(silent = false): Promise<void> {
  await refreshStore(silent, () => desktop.getBackendLogs(), (logs) => {
    backendLogs.set(logs);
  });
}

export async function refreshGamesList(silent = false): Promise<boolean> {
  return attemptRefresh(silent, async () => {
    const result = await listGames();
    gamesList.set(result.games);

    const currentGame = get(activeGame);
    if (currentGame && !result.games.some((game) => game.game_id === currentGame.game_id)) {
      activeGame.set(null);
      legalMoves.set([]);
      selectedSquare.set(null);
      boardAscii.set('');
      updateDesktopState((state) => ({ ...state, lastGameId: null }));
    }
  });
}

export async function refreshArchive(silent = false): Promise<boolean> {
  return attemptRefresh(silent, async () => {
    const result = await listArchived();
    archivedList.set(result.games);
    if (result.storage) {
      storageStats.set(result.storage);
    }
  });
}

export async function refreshAnalysisJobs(silent = false): Promise<boolean> {
  return refreshStore(silent, listAnalysisJobs, (result) => {
    analysisJobs.set(result.jobs);
  });
}

async function updateAnalysisProgress(job: AnalysisJob | null): Promise<void> {
  if (!job || !isInProgressStatus(job.status)) {
    await desktop.setProgressBar(null);
    return;
  }

  const { moves_analyzed, total_moves } = job.status.InProgress;
  if (total_moves <= 0) {
    await desktop.setProgressBar(null);
    return;
  }

  await desktop.setProgressBar(moves_analyzed / total_moves);
}

async function refreshActiveAnalysisJob(silent = false): Promise<boolean> {
  const currentJob = get(activeAnalysis);
  if (!currentJob) {
    return false;
  }

  return attemptRefresh(silent, async () => {
    const previousJob = currentJob;
    const nextJob = await getAnalysisJob(currentJob.id);
    activeAnalysis.set(nextJob);
    await updateAnalysisProgress(nextJob);

    if (!isTerminalAnalysisStatus(previousJob.status) && isTerminalAnalysisStatus(nextJob.status)) {
      await refreshAnalysisJobs(true);
      if (nextJob.status === 'Completed') {
        pushToast('Analysis completed');
        await notifyIfEnabled(
          'CheckAI Desktop',
          `Analysis ${shortId(nextJob.id)} completed successfully.`
        );
      } else if (nextJob.status === 'Cancelled') {
        pushToast('Analysis cancelled');
      } else if (isFailedStatus(nextJob.status)) {
        pushError(nextJob.status.Failed.error);
      }
    }
  });
}

async function refreshBoardAscii(gameId: string): Promise<void> {
  try {
    boardAscii.set(await getBoardAscii(gameId));
  } catch {
    boardAscii.set('Board ASCII preview unavailable.');
  }
}

async function refreshActiveGame(silent = false): Promise<boolean> {
  const game = get(activeGame);
  if (!game) {
    return false;
  }

  return attemptRefresh(silent, async () => {
    const nextGame = await getGame(game.game_id);
    activeGame.set(nextGame);

    if (!nextGame.is_over) {
      const moveResult = await getLegalMoves(nextGame.game_id);
      legalMoves.set(moveResult.moves);
    } else {
      legalMoves.set([]);
    }

    await refreshBoardAscii(nextGame.game_id);
  });
}

function stopWorkspaceRefreshPolling(): void {
  if (workspaceRefreshTimer) {
    clearInterval(workspaceRefreshTimer);
    workspaceRefreshTimer = null;
  }
}

function startWorkspaceRefreshPolling(): void {
  if (workspaceRefreshTimer) {
    return;
  }

  workspaceRefreshTimer = setInterval(async () => {
    if (workspaceRefreshInFlight || Date.now() < workspacePollingBackoffUntil) {
      return;
    }

    workspaceRefreshInFlight = true;
    try {
      const refreshResults = [
        await refreshGamesList(true),
        await refreshArchive(true),
        await refreshAnalysisJobs(true),
        await refreshActiveGame(true),
      ];
      if (!analysisPollTimer) {
        refreshResults.push(await refreshActiveAnalysisJob(true));
      }

      if (refreshResults.some(Boolean)) {
        workspacePollingFailureCount = 0;
        workspacePollingBackoffUntil = 0;
      } else {
        workspacePollingFailureCount += 1;
        const backoffMultiplier = Math.min(
          workspacePollingFailureCount,
          MAX_WORKSPACE_REFRESH_BACKOFF_MULTIPLIER
        );
        workspacePollingBackoffUntil =
          Date.now() + WORKSPACE_REFRESH_INTERVAL_MS * backoffMultiplier;
      }
    } finally {
      workspaceRefreshInFlight = false;
    }
  }, WORKSPACE_REFRESH_INTERVAL_MS);
}

function stopAnalysisPolling(): void {
  if (analysisPollTimer) {
    clearInterval(analysisPollTimer);
    analysisPollTimer = null;
  }
  analysisPollingSession += 1;
  analysisRefreshInFlight = false;
  void desktop.setProgressBar(null);
}

function startAnalysisPolling(jobId: string): void {
  stopAnalysisPolling();
  const pollingSession = analysisPollingSession;
  analysisPollTimer = setInterval(async () => {
    if (pollingSession !== analysisPollingSession) {
      return;
    }

    if (analysisRefreshInFlight) {
      return;
    }

    const currentJob = get(activeAnalysis);
    if (!currentJob || currentJob.id !== jobId) {
      stopAnalysisPolling();
      return;
    }

    analysisRefreshInFlight = true;
    try {
      await refreshActiveAnalysisJob(true);
      const nextJob = get(activeAnalysis);
      if (!nextJob || isTerminalAnalysisStatus(nextJob.status)) {
        stopAnalysisPolling();
      }
    } finally {
      if (pollingSession === analysisPollingSession) {
        analysisRefreshInFlight = false;
      }
    }
  }, ANALYSIS_POLL_INTERVAL_MS);
}

export async function refreshWorkspaceData(silent = false): Promise<void> {
  await Promise.all([
    refreshBackendState(silent),
    refreshUpdateState(silent),
    refreshLogs(silent),
    refreshGamesList(silent),
    refreshArchive(silent),
    refreshAnalysisJobs(silent),
  ]);
}

export async function initializeDesktopWorkspace(): Promise<() => void> {
  await loadDesktopState();
  applyDesktopState(get(desktopState), false);
  await refreshWorkspaceData(true);

  const { lastGameId } = get(desktopState);
  if (lastGameId) {
    await openGame(lastGameId, { keepCurrentView: true, silent: true });
  }

  startWorkspaceRefreshPolling();

  if (!menuCommandCleanup) {
    menuCommandCleanup = desktop.onMenuCommand((command) => {
      void handleDesktopCommand(command);
    });
  }

  const handleSystemThemeChange = () => {
    if (get(desktopState).theme === 'system') {
      syncDocumentTheme('system');
    }
  };

  systemThemeMedia?.addEventListener('change', handleSystemThemeChange);

  return () => {
    stopWorkspaceRefreshPolling();
    stopAnalysisPolling();
    clearNotificationTimers();
    if (menuCommandCleanup) {
      menuCommandCleanup();
      menuCommandCleanup = null;
    }
    systemThemeMedia?.removeEventListener('change', handleSystemThemeChange);
    void desktop.setProgressBar(null);
  };
}

export async function createNewGame(): Promise<void> {
  try {
    const result = await createGame();
    await refreshGamesList(true);
    await openGame(result.game_id);
    pushToast('New game created');
  } catch (error) {
    pushError(error instanceof Error ? error.message : String(error));
  }
}

export async function importFenText(rawText: string): Promise<void> {
  const fen = rawText.trim();
  if (!fen) {
    pushError('Paste or drop a FEN string first.');
    return;
  }

  try {
    const result = await importFen(fen);
    fenInput.set('');
    await refreshGamesList(true);
    await openGame(result.game_id);
    pushToast('Game imported from FEN');
  } catch (error) {
    pushError(error instanceof Error ? error.message : String(error));
  }
}

export async function importFenFromField(): Promise<void> {
  await importFenText(get(fenInput));
}

export async function importFenFromFile(): Promise<void> {
  try {
    const filePath = await desktop.pickFile();
    if (!filePath) {
      return;
    }

    const text = await desktop.readTextFile(filePath);
    await importFenText(text);
  } catch (error) {
    pushError(error instanceof Error ? error.message : String(error));
  }
}

export async function openGame(
  gameId: string,
  options: { keepCurrentView?: boolean; silent?: boolean } = {}
): Promise<void> {
  try {
    const game = await getGame(gameId);
    activeGame.set(game);
    selectedSquare.set(null);
    replayState.set(null);
    replayGameId.set(null);
    updateDesktopState((state) => ({ ...state, lastGameId: gameId }));

    if (!game.is_over) {
      const moveResult = await getLegalMoves(game.game_id);
      legalMoves.set(moveResult.moves);
    } else {
      legalMoves.set([]);
    }

    await refreshBoardAscii(game.game_id);

    if (!options.keepCurrentView) {
      navigateTo('board');
    }
  } catch (error) {
    if (!options.silent) {
      pushError(error instanceof Error ? error.message : String(error));
    }
  }
}

export async function deleteGameById(gameId: string): Promise<void> {
  const confirmed = await confirmAction({
    title: 'Delete active game',
    message: `Delete game ${shortId(gameId)}…? This removes the live session from the desktop workspace.`,
    confirmLabel: 'Delete game',
  });

  if (!confirmed) {
    return;
  }

  try {
    await deleteGame(gameId);
    const currentGame = get(activeGame);
    if (currentGame?.game_id === gameId) {
      activeGame.set(null);
      legalMoves.set([]);
      selectedSquare.set(null);
      boardAscii.set('');
      updateDesktopState((state) => ({ ...state, lastGameId: null }));
    }
    await refreshGamesList(true);
    pushToast('Game deleted');
  } catch (error) {
    pushError(error instanceof Error ? error.message : String(error));
  }
}

export async function handleBoardSquareClick(square: string): Promise<void> {
  const game = get(activeGame);
  if (!game || game.is_over) {
    return;
  }

  const currentlySelected = get(selectedSquare);
  const moves = get(legalMoves);
  if (currentlySelected) {
    const move = moves.find(
      (candidate) => candidate.from === currentlySelected && candidate.to === square
    );

    if (move) {
      try {
        const response = await submitMove(
          game.game_id,
          currentlySelected,
          square,
          move.promotion
        );
        if (!response.success) {
          pushError(response.message);
          return;
        }
        await openGame(game.game_id, { keepCurrentView: true });
      } catch (error) {
        pushError(error instanceof Error ? error.message : String(error));
      } finally {
        selectedSquare.set(null);
      }
      return;
    }
  }

  const piece = game.state.board[square];
  if (piece) {
    const isWhitePiece = piece === piece.toUpperCase();
    const isWhiteTurn = game.state.turn === 'white';
    if (isWhitePiece === isWhiteTurn) {
      selectedSquare.set(square);
      return;
    }
  }

  selectedSquare.set(null);
}

export async function copyActiveFen(): Promise<void> {
  const game = get(activeGame);
  if (!game) {
    return;
  }

  try {
    const result = await exportFen(game.game_id);
    await copyText(result.fen, 'FEN copied to clipboard');
  } catch (error) {
    pushError(error instanceof Error ? error.message : String(error));
  }
}

export async function saveActiveFen(): Promise<void> {
  const game = get(activeGame);
  if (!game) {
    return;
  }

  try {
    const result = await exportFen(game.game_id);
    await saveTextExport(
      `${slugify(game.game_id)}.fen`,
      result.fen,
      [{ name: 'FEN position', extensions: ['fen', 'txt'] }],
      'FEN saved to disk'
    );
  } catch (error) {
    pushError(error instanceof Error ? error.message : String(error));
  }
}

export async function copyActivePgn(): Promise<void> {
  const game = get(activeGame);
  if (!game) {
    return;
  }

  try {
    const pgn = await exportPgn(game.game_id);
    await copyText(pgn, 'PGN copied to clipboard');
  } catch (error) {
    pushError(error instanceof Error ? error.message : String(error));
  }
}

export async function saveActivePgn(): Promise<void> {
  const game = get(activeGame);
  if (!game) {
    return;
  }

  try {
    const pgn = await exportPgn(game.game_id);
    await saveTextExport(
      `${slugify(game.game_id)}.pgn`,
      pgn,
      [{ name: 'PGN game', extensions: ['pgn', 'txt'] }],
      'PGN saved to disk'
    );
  } catch (error) {
    pushError(error instanceof Error ? error.message : String(error));
  }
}

export async function openBackendInBrowser(): Promise<void> {
  const url = get(desktopState).backendUrl.trim();
  if (!url) {
    return;
  }

  try {
    await desktop.openExternal(url);
  } catch (error) {
    pushError(error instanceof Error ? error.message : String(error));
  }
}

export async function openArchivedReplay(gameId: string): Promise<void> {
  try {
    replayGameId.set(gameId);
    replayState.set(await replayArchived(gameId, 0));
    navigateTo('archive');
  } catch (error) {
    pushError(error instanceof Error ? error.message : String(error));
  }
}

export async function replayTo(moveNumber: number): Promise<void> {
  const gameId = get(replayGameId);
  if (!gameId) {
    return;
  }

  try {
    replayState.set(await replayArchived(gameId, moveNumber));
  } catch (error) {
    pushError(error instanceof Error ? error.message : String(error));
  }
}

export function closeReplay(): void {
  replayState.set(null);
  replayGameId.set(null);
}

export function setAnalysisDepth(value: number): void {
  const normalized = Math.min(
    MAX_ANALYSIS_DEPTH,
    Math.max(MIN_ANALYSIS_DEPTH, Math.round(value || MIN_ANALYSIS_DEPTH))
  );
  analysisDepth.set(normalized);
}

export async function submitAnalysisForGame(gameId?: string): Promise<void> {
  const targetGameId = gameId ?? get(activeGame)?.game_id ?? null;
  if (!targetGameId) {
    pushError('Open a game before starting analysis.');
    return;
  }

  try {
    const result = await startAnalysis(targetGameId, get(analysisDepth));
    pushToast('Analysis started');
    await refreshAnalysisJobs(true);
    navigateTo('analysis');
    await viewAnalysisJob(result.job_id);
  } catch (error) {
    pushError(error instanceof Error ? error.message : String(error));
  }
}

export async function viewAnalysisJob(jobId: string): Promise<void> {
  try {
    const job = await getAnalysisJob(jobId);
    activeAnalysis.set(job);
    navigateTo('analysis');
    await updateAnalysisProgress(job);
    if (isTerminalAnalysisStatus(job.status)) {
      stopAnalysisPolling();
      await refreshAnalysisJobs(true);
    } else {
      startAnalysisPolling(jobId);
    }
  } catch (error) {
    pushError(error instanceof Error ? error.message : String(error));
  }
}

export async function cancelAnalysis(jobId: string): Promise<void> {
  try {
    await cancelAnalysisJob(jobId);
    pushToast('Analysis cancelled');
    await refreshAnalysisJobs(true);
    if (get(activeAnalysis)?.id === jobId) {
      await viewAnalysisJob(jobId);
    }
  } catch (error) {
    pushError(error instanceof Error ? error.message : String(error));
  }
}

export async function startBackendProcess(): Promise<void> {
  try {
    const status = await desktop.startBackend(get(desktopState));
    backendStatus.set(status);
    if (status.running) {
      pushToast('Backend started successfully');
      await refreshWorkspaceData(true);
    }
  } catch (error) {
    pushError(error instanceof Error ? error.message : String(error));
  }
}

export async function stopBackendProcess(): Promise<void> {
  try {
    const status = await desktop.stopBackend();
    backendStatus.set(status);
    pushToast('Backend stopped');
    stopAnalysisPolling();
  } catch (error) {
    pushError(error instanceof Error ? error.message : String(error));
  }
}

export async function checkForDesktopUpdates(): Promise<void> {
  try {
    updateStatus.set(await desktop.checkForUpdates());
  } catch (error) {
    pushError(error instanceof Error ? error.message : String(error));
  }
}

export async function downloadDesktopUpdate(): Promise<void> {
  try {
    updateStatus.set(await desktop.downloadUpdate());
  } catch (error) {
    pushError(error instanceof Error ? error.message : String(error));
  }
}

export async function installDesktopUpdate(): Promise<void> {
  try {
    await desktop.installUpdate();
  } catch (error) {
    pushError(error instanceof Error ? error.message : String(error));
  }
}

let presetIdCounter = 0;

function createPresetId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return `preset-${crypto.randomUUID()}`;
  }

  presetIdCounter += 1;
  return `preset-${Date.now()}-${presetIdCounter}`;
}

function currentPresetFromState(name: string): BackendPreset {
  const state = get(desktopState);
  return {
    id: createPresetId(),
    name,
    backendExecutable: state.backendExecutable,
    backendArgs: state.backendArgs,
    backendWorkingDirectory: state.backendWorkingDirectory,
    backendUrl: state.backendUrl,
    openingBookPath: state.openingBookPath,
    tablebasePath: state.tablebasePath,
    autoStartBackend: state.autoStartBackend,
    createdAt: Date.now(),
  };
}

export async function saveCurrentPreset(): Promise<void> {
  const name =
    (await promptForValue({
      title: 'Save backend preset',
      message: 'Give the current backend configuration a reusable preset name.',
      confirmLabel: 'Save preset',
      placeholder: 'Analysis workspace',
    })) ?? '';
  if (!name) {
    return;
  }

  const preset = currentPresetFromState(name);
  updateDesktopState((state) => ({
    ...state,
    backendPresets: [preset, ...state.backendPresets.filter((entry) => entry.name !== name)],
  }));
  pushToast(`Preset “${name}” saved`);
}

export function loadPresetIntoState(id: string): void {
  const preset = get(desktopState).backendPresets.find((entry) => entry.id === id);
  if (!preset) {
    return;
  }

  updateDesktopState((state) => ({
    ...state,
    backendExecutable: preset.backendExecutable,
    backendArgs: preset.backendArgs,
    backendWorkingDirectory: preset.backendWorkingDirectory,
    backendUrl: preset.backendUrl,
    openingBookPath: preset.openingBookPath,
    tablebasePath: preset.tablebasePath,
    autoStartBackend: preset.autoStartBackend,
  }));
  pushToast(`Preset “${preset.name}” loaded`);
}

export async function deletePreset(id: string): Promise<void> {
  const preset = get(desktopState).backendPresets.find((entry) => entry.id === id);
  if (!preset) {
    return;
  }

  const confirmed = await confirmAction({
    title: 'Delete backend preset',
    message: `Remove preset “${preset.name}” from the desktop workspace?`,
    confirmLabel: 'Delete preset',
  });

  if (!confirmed) {
    return;
  }

  updateDesktopState((state) => ({
    ...state,
    backendPresets: state.backendPresets.filter((entry) => entry.id !== id),
  }));
  pushToast(`Preset “${preset.name}” removed`);
}

export async function pickBackendExecutable(): Promise<void> {
  try {
    const file = await desktop.pickFile();
    if (!file) {
      return;
    }
    updateDesktopState((state) => ({ ...state, backendExecutable: file }));
  } catch (error) {
    pushError(error instanceof Error ? error.message : String(error));
  }
}

export async function pickWorkingDirectory(): Promise<void> {
  try {
    const directory = await desktop.pickDirectory();
    if (!directory) {
      return;
    }
    updateDesktopState((state) => ({
      ...state,
      backendWorkingDirectory: directory,
      recentWorkspaces: [directory, ...state.recentWorkspaces.filter((entry) => entry !== directory)]
        .slice(0, 10),
    }));
    pushToast('Working directory updated');
  } catch (error) {
    pushError(error instanceof Error ? error.message : String(error));
  }
}

export async function pickOpeningBook(): Promise<void> {
  try {
    const file = await desktop.pickFile();
    if (!file) {
      return;
    }
    updateDesktopState((state) => ({ ...state, openingBookPath: file }));
  } catch (error) {
    pushError(error instanceof Error ? error.message : String(error));
  }
}

export async function pickTablebaseDirectory(): Promise<void> {
  try {
    const directory = await desktop.pickDirectory();
    if (!directory) {
      return;
    }
    updateDesktopState((state) => ({ ...state, tablebasePath: directory }));
  } catch (error) {
    pushError(error instanceof Error ? error.message : String(error));
  }
}

export async function openWorkingDirectory(): Promise<void> {
  const directory = get(desktopState).backendWorkingDirectory.trim();
  if (!directory) {
    pushError('Choose a working directory first.');
    return;
  }

  try {
    await desktop.openPath(directory);
  } catch (error) {
    pushError(error instanceof Error ? error.message : String(error));
  }
}

export async function refreshCurrentView(): Promise<void> {
  switch (get(currentView)) {
    case 'dashboard':
      await refreshWorkspaceData(true);
      await refreshActiveGame(true);
      break;
    case 'games':
      await refreshGamesList();
      break;
    case 'board':
      await refreshActiveGame();
      break;
    case 'archive':
      await refreshArchive();
      break;
    case 'analysis':
      await refreshAnalysisJobs();
      await refreshActiveAnalysisJob(true);
      break;
    case 'logs':
      await refreshLogs();
      break;
    case 'engine':
    case 'settings':
      await refreshBackendState(true);
      await refreshUpdateState(true);
      break;
  }
}

export async function handleDesktopCommand(command: string): Promise<void> {
  switch (command) {
    case 'new-game':
      await createNewGame();
      break;
    case 'import-fen-file':
      await importFenFromFile();
      break;
    case 'export-fen':
      await copyActiveFen();
      break;
    case 'save-fen':
      await saveActiveFen();
      break;
    case 'export-pgn':
      await copyActivePgn();
      break;
    case 'save-pgn':
      await saveActivePgn();
      break;
    case 'nav:dashboard':
      navigateTo('dashboard');
      break;
    case 'nav:games':
      navigateTo('games');
      break;
    case 'nav:board':
      navigateTo('board');
      break;
    case 'nav:archive':
      navigateTo('archive');
      break;
    case 'nav:analysis':
      navigateTo('analysis');
      break;
    case 'nav:engine':
      navigateTo('engine');
      break;
    case 'nav:logs':
      navigateTo('logs');
      break;
    case 'nav:settings':
      navigateTo('settings');
      break;
    case 'open-command-palette':
      paletteOpen.set(true);
      break;
    case 'start-backend':
      await startBackendProcess();
      break;
    case 'stop-backend':
      await stopBackendProcess();
      break;
    case 'open-working-directory':
      await openWorkingDirectory();
      break;
    case 'check-for-updates':
      await checkForDesktopUpdates();
      break;
  }
}
