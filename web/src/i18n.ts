// ============================================================================
// CheckAI Web UI — Internationalization
// ============================================================================

// All translations are embedded inline to keep the app zero-build capable.
// Supported: en, de, fr, es, zh-CN, ja, pt, ru

const DICTIONARIES: Record<string, Record<string, string>> = {
  en: {
    'app.title': 'CheckAI — Chess for AI Agents',
    'nav.dashboard': 'Dashboard',
    'nav.game': 'Game',
    'nav.archive': 'Archive',
    'ws.connected': 'Connected',
    'ws.disconnected': 'Disconnected',
    'dashboard.active_games': 'Active Games',
    'dashboard.new_game': 'New Game',
    'dashboard.no_games': 'No games yet.',
    'dashboard.no_games_hint': 'Create a new game to get started.',
    'dashboard.storage_stats': 'Storage Statistics',
    'dashboard.refresh': 'Refresh',
    'stat.active_games': 'Active Games',
    'stat.archived_games': 'Archived Games',
    'stat.active_storage': 'Active Storage',
    'stat.archive_storage': 'Archive Storage',
    'game.no_game_selected': 'No game selected.',
    'game.no_game_hint': 'Select a game from the dashboard or create a new one.',
    'game.black': 'Black',
    'game.white': 'White',
    'game.promotion_title': 'Pawn Promotion',
    'game.info': 'Game Info',
    'game.game_id': 'Game ID',
    'game.turn': 'Turn',
    'game.move_number': 'Move Number',
    'game.status': 'Status',
    'game.check': 'Check',
    'game.legal_moves': 'Legal Moves',
    'game.actions': 'Actions',
    'game.resign': 'Resign',
    'game.offer_draw': 'Offer Draw',
    'game.claim_draw': 'Claim Draw',
    'game.delete': 'Delete Game',
    'game.move_input': 'Enter Move',
    'game.from_placeholder': 'from (e.g. e2)',
    'game.to_placeholder': 'to (e.g. e4)',
    'game.submit_move': 'Move',
    'game.move_history': 'Move History',
    'game.no_moves': 'No moves yet.',
    'game.castling_rights': 'Castling Rights',
    'game.kingside': 'Kingside',
    'game.queenside': 'Queenside',
    'game.turn_white': '♔ White',
    'game.turn_black': '♚ Black',
    'game.status_over': 'Finished',
    'game.status_active': 'In Progress',
    'game.check_yes': '⚠ Yes!',
    'game.check_no': 'No',
    'game.your_turn': '⏱ Your turn',
    'game.badge_over': 'Finished',
    'game.badge_active': 'Active',
    'game.flip_board': 'Flip Board',
    'game.fen_export': 'Copy FEN',
    'game.pgn_export': 'Copy PGN',
    'game.fen_import': 'Import FEN',
    'result.white_wins': '1-0 White wins',
    'result.black_wins': '0-1 Black wins',
    'result.draw': '½-½ Draw',
    'reason.Checkmate': 'Checkmate',
    'reason.Stalemate': 'Stalemate',
    'reason.ThreefoldRepetition': 'Threefold Repetition',
    'reason.FivefoldRepetition': 'Fivefold Repetition',
    'reason.FiftyMoveRule': 'Fifty-Move Rule',
    'reason.SeventyFiveMoveRule': 'Seventy-Five-Move Rule',
    'reason.InsufficientMaterial': 'Insufficient Material',
    'reason.Resignation': 'Resignation',
    'reason.DrawAgreement': 'Draw by Agreement',
    'piece.K': 'King',
    'piece.Q': 'Queen',
    'piece.R': 'Rook',
    'piece.B': 'Bishop',
    'piece.N': 'Knight',
    'piece.P': 'Pawn',
    'archive.title': 'Game Archive',
    'archive.refresh': '↻ Refresh',
    'archive.no_games': 'No archived games yet.',
    'archive.badge': 'Archive',
    'archive.half_moves': '%{n} half-moves',
    'archive.replay': 'Replay',
    'archive.replay_title': 'Replay — %{id}',
    'archive.close': '✕ Close',
    'archive.move_label': 'Move',
    'toast.new_game_created': 'New game created!',
    'toast.game_deleted': 'Game deleted',
    'toast.current_game_deleted': 'Current game was deleted',
    'toast.invalid_move': 'Invalid move: %{error}',
    'toast.error': 'Error: %{error}',
    'toast.load_archive_failed': 'Failed to load archive: %{error}',
    'toast.replay_failed': 'Replay failed: %{error}',
    'toast.fen_copied': 'FEN copied to clipboard',
    'toast.pgn_copied': 'PGN copied to clipboard',
    'toast.fen_import_prompt': 'Enter a FEN string:',
    'toast.fen_imported': 'Game created from FEN',
    'toast.enter_from_to': 'Please enter "from" and "to" squares',
    'confirm.resign': 'Really resign?',
    'confirm.delete': 'Really delete this game?',
    'confirm.claim_draw_reason': 'Enter reason: threefold_repetition or fifty_move_rule',
    'analysis.title': 'Engine Analysis',
    'analysis.start': 'Analyze',
    'analysis.stop': 'Stop',
    'analysis.depth': 'Depth',
    'analysis.status': 'Status',
    'analysis.progress': 'Progress',
    'analysis.summary': 'Summary',
    'analysis.summary.best': 'Best',
    'analysis.summary.excellent': 'Excellent',
    'analysis.summary.good': 'Good',
    'analysis.summary.inaccuracies': 'Inaccuracies',
    'analysis.summary.mistakes': 'Mistakes',
    'analysis.summary.blunders': 'Blunders',
    'analysis.summary.book': 'Book',
    'analysis.total_moves': 'Total Moves',
    'analysis.avg_cp_loss': 'Average CP Loss',
    'analysis.white_accuracy': 'White Accuracy',
    'analysis.black_accuracy': 'Black Accuracy',
    'analysis.white_avg_cp_loss': 'White Avg CP Loss',
    'analysis.black_avg_cp_loss': 'Black Avg CP Loss',
    'analysis.book_available': 'Book',
    'analysis.tablebase_available': 'Tablebase',
    'analysis.available': 'Available',
    'analysis.unavailable': 'Unavailable',
    'analysis.queued': 'Queued',
    'analysis.running': 'Running',
    'analysis.completed': 'Completed',
    'analysis.cancelled': 'Cancelled',
    'analysis.failed': 'Failed',
    'analysis.score': 'Score',
    'analysis.best_move': 'Best Move',
    'analysis.pv': 'Principal Variation',
    'analysis.nodes': 'Nodes',
    'analysis.nps': 'NPS',
    'analysis.time': 'Time',
    'analysis.idle': 'Press Analyze to start engine analysis.',
    'lang.label': 'Language',
  },

  de: {
    'app.title': 'CheckAI — Schach für KI-Agenten',
    'nav.dashboard': 'Dashboard',
    'nav.game': 'Spiel',
    'nav.archive': 'Archiv',
    'ws.connected': 'Verbunden',
    'ws.disconnected': 'Getrennt',
    'dashboard.active_games': 'Aktive Spiele',
    'dashboard.new_game': 'Neues Spiel',
    'dashboard.no_games': 'Noch keine Spiele vorhanden.',
    'dashboard.no_games_hint': 'Erstelle ein neues Spiel um zu beginnen.',
    'dashboard.storage_stats': 'Speicherstatistik',
    'dashboard.refresh': 'Aktualisieren',
    'stat.active_games': 'Aktive Spiele',
    'stat.archived_games': 'Archivierte Spiele',
    'stat.active_storage': 'Aktiver Speicher',
    'stat.archive_storage': 'Archiv-Speicher',
    'game.no_game_selected': 'Kein Spiel ausgewählt.',
    'game.no_game_hint': 'Wähle ein Spiel aus dem Dashboard oder erstelle ein neues.',
    'game.black': 'Schwarz',
    'game.white': 'Weiß',
    'game.promotion_title': 'Bauernumwandlung',
    'game.info': 'Spielinfo',
    'game.game_id': 'Spiel-ID',
    'game.turn': 'Am Zug',
    'game.move_number': 'Zugnummer',
    'game.status': 'Status',
    'game.check': 'Schach',
    'game.legal_moves': 'Legale Züge',
    'game.actions': 'Aktionen',
    'game.resign': 'Aufgeben',
    'game.offer_draw': 'Remis anbieten',
    'game.claim_draw': 'Remis beanspruchen',
    'game.delete': 'Spiel löschen',
    'game.move_input': 'Zug eingeben',
    'game.from_placeholder': 'von (z.B. e2)',
    'game.to_placeholder': 'nach (z.B. e4)',
    'game.submit_move': 'Ziehen',
    'game.move_history': 'Zughistorie',
    'game.no_moves': 'Noch keine Züge.',
    'game.castling_rights': 'Rochaderechte',
    'game.kingside': 'Königsseite',
    'game.queenside': 'Damenseite',
    'game.turn_white': '♔ Weiß',
    'game.turn_black': '♚ Schwarz',
    'game.status_over': 'Beendet',
    'game.status_active': 'Läuft',
    'game.check_yes': '⚠ Ja!',
    'game.check_no': 'Nein',
    'game.your_turn': '⏱ Am Zug',
    'game.badge_over': 'Beendet',
    'game.badge_active': 'Aktiv',
    'game.flip_board': 'Brett drehen',
    'game.fen_export': 'FEN kopieren',
    'game.pgn_export': 'PGN kopieren',
    'game.fen_import': 'FEN importieren',
    'result.white_wins': '1-0 Weiß gewinnt',
    'result.black_wins': '0-1 Schwarz gewinnt',
    'result.draw': '½-½ Remis',
    'reason.Checkmate': 'Schachmatt',
    'reason.Stalemate': 'Patt',
    'reason.ThreefoldRepetition': 'Dreifache Stellungswiederholung',
    'reason.FivefoldRepetition': 'Fünffache Wiederholung',
    'reason.FiftyMoveRule': '50-Züge-Regel',
    'reason.SeventyFiveMoveRule': '75-Züge-Regel',
    'reason.InsufficientMaterial': 'Ungenügendes Material',
    'reason.Resignation': 'Aufgabe',
    'reason.DrawAgreement': 'Remis durch Vereinbarung',
    'piece.K': 'König',
    'piece.Q': 'Dame',
    'piece.R': 'Turm',
    'piece.B': 'Läufer',
    'piece.N': 'Springer',
    'piece.P': 'Bauer',
    'archive.title': 'Spielarchiv',
    'archive.refresh': '↻ Aktualisieren',
    'archive.no_games': 'Noch keine archivierten Spiele.',
    'archive.badge': 'Archiv',
    'archive.half_moves': '%{n} Halbzüge',
    'archive.replay': 'Wiedergabe',
    'archive.replay_title': 'Wiedergabe — %{id}',
    'archive.close': '✕ Schließen',
    'archive.move_label': 'Zug',
    'toast.new_game_created': 'Neues Spiel erstellt!',
    'toast.game_deleted': 'Spiel gelöscht',
    'toast.current_game_deleted': 'Aktuelles Spiel wurde gelöscht',
    'toast.invalid_move': 'Ungültiger Zug: %{error}',
    'toast.error': 'Fehler: %{error}',
    'toast.load_archive_failed': 'Archiv laden fehlgeschlagen: %{error}',
    'toast.replay_failed': 'Wiedergabe fehlgeschlagen: %{error}',
    'toast.fen_copied': 'FEN in die Zwischenablage kopiert',
    'toast.pgn_copied': 'PGN in die Zwischenablage kopiert',
    'toast.fen_import_prompt': 'FEN-String eingeben:',
    'toast.fen_imported': 'Spiel aus FEN erstellt',
    'toast.enter_from_to': 'Bitte "von" und "nach" Feld angeben',
    'confirm.resign': 'Wirklich aufgeben?',
    'confirm.delete': 'Dieses Spiel wirklich löschen?',
    'confirm.claim_draw_reason': 'Grund angeben: threefold_repetition oder fifty_move_rule',
    'analysis.title': 'Engine-Analyse',
    'analysis.start': 'Analysieren',
    'analysis.stop': 'Stoppen',
    'analysis.summary.best': 'Beste',
    'analysis.summary.excellent': 'Exzellent',
    'analysis.summary.good': 'Gut',
    'analysis.summary.inaccuracies': 'Ungenauigkeiten',
    'analysis.summary.mistakes': 'Fehler',
    'analysis.summary.blunders': 'Patzer',
    'analysis.summary.book': 'Buch',
    'analysis.depth': 'Tiefe',
    'analysis.score': 'Bewertung',
    'analysis.best_move': 'Bester Zug',
    'analysis.pv': 'Hauptvariante',
    'analysis.nodes': 'Knoten',
    'analysis.nps': 'Knoten/s',
    'analysis.time': 'Zeit',
    'analysis.idle': 'Analysieren drücken um die Engine-Analyse zu starten.',
    'lang.label': 'Sprache',
  },

  fr: {
    'app.title': 'CheckAI — Échecs pour agents IA',
    'nav.dashboard': 'Tableau de bord',
    'nav.game': 'Partie',
    'nav.archive': 'Archives',
    'ws.connected': 'Connecté',
    'ws.disconnected': 'Déconnecté',
    'dashboard.active_games': 'Parties actives',
    'dashboard.new_game': 'Nouvelle partie',
    'dashboard.no_games': 'Aucune partie pour le moment.',
    'dashboard.no_games_hint': 'Créez une nouvelle partie pour commencer.',
    'dashboard.storage_stats': 'Statistiques de stockage',
    'game.no_game_selected': 'Aucune partie sélectionnée.',
    'game.info': 'Infos partie',
    'game.turn': 'Au trait',
    'game.resign': 'Abandonner',
    'game.move_history': 'Historique',
    'game.flip_board': 'Retourner',
    'game.fen_export': 'Copier FEN',
    'game.pgn_export': 'Copier PGN',
    'game.fen_import': 'Importer FEN',
    'result.white_wins': '1-0 Les blancs gagnent',
    'result.black_wins': '0-1 Les noirs gagnent',
    'result.draw': '½-½ Nulle',
    'reason.Checkmate': 'Échec et mat',
    'archive.title': 'Archives des parties',
    'analysis.title': 'Analyse moteur',
    'analysis.start': 'Analyser',
    'analysis.stop': 'Arrêter',
    'analysis.summary.best': 'Meilleurs',
    'analysis.summary.excellent': 'Excellents',
    'analysis.summary.good': 'Bons',
    'analysis.summary.inaccuracies': 'Imprécisions',
    'analysis.summary.mistakes': 'Erreurs',
    'analysis.summary.blunders': 'Gaffes',
    'analysis.summary.book': 'Livre',
    'lang.label': 'Langue',
  },

  es: {
    'app.title': 'CheckAI — Ajedrez para agentes de IA',
    'nav.dashboard': 'Panel',
    'nav.game': 'Partida',
    'nav.archive': 'Archivo',
    'ws.connected': 'Conectado',
    'ws.disconnected': 'Desconectado',
    'dashboard.active_games': 'Partidas activas',
    'dashboard.new_game': 'Nueva partida',
    'game.resign': 'Rendirse',
    'game.flip_board': 'Girar tablero',
    'result.white_wins': '1-0 Ganan blancas',
    'result.black_wins': '0-1 Ganan negras',
    'result.draw': '½-½ Tablas',
    'reason.Checkmate': 'Jaque mate',
    'analysis.title': 'Análisis del motor',
    'analysis.start': 'Analizar',
    'analysis.stop': 'Detener',
    'analysis.summary.best': 'Mejores',
    'analysis.summary.excellent': 'Excelentes',
    'analysis.summary.good': 'Buenas',
    'analysis.summary.inaccuracies': 'Imprecisiones',
    'analysis.summary.mistakes': 'Errores',
    'analysis.summary.blunders': 'Blunders',
    'analysis.summary.book': 'Libro',
    'lang.label': 'Idioma',
  },

  'zh-CN': {
    'app.title': 'CheckAI — AI代理国际象棋',
    'nav.dashboard': '仪表盘',
    'nav.game': '对局',
    'nav.archive': '存档',
    'ws.connected': '已连接',
    'ws.disconnected': '已断开',
    'dashboard.active_games': '活跃对局',
    'dashboard.new_game': '新建对局',
    'game.resign': '认输',
    'game.flip_board': '翻转棋盘',
    'result.white_wins': '1-0 白方胜',
    'result.black_wins': '0-1 黑方胜',
    'result.draw': '½-½ 和棋',
    'reason.Checkmate': '将杀',
    'analysis.title': '引擎分析',
    'analysis.start': '分析',
    'analysis.stop': '停止',
    'analysis.summary.best': '最佳',
    'analysis.summary.excellent': '优秀',
    'analysis.summary.good': '良好',
    'analysis.summary.inaccuracies': '不精确',
    'analysis.summary.mistakes': '错误',
    'analysis.summary.blunders': '漏着',
    'analysis.summary.book': '库',
    'lang.label': '语言',
  },

  ja: {
    'app.title': 'CheckAI — AIエージェント用チェス',
    'nav.dashboard': 'ダッシュボード',
    'nav.game': '対局',
    'nav.archive': 'アーカイブ',
    'ws.connected': '接続済み',
    'ws.disconnected': '切断',
    'game.resign': '投了',
    'game.flip_board': '盤面反転',
    'result.white_wins': '1-0 白勝ち',
    'result.black_wins': '0-1 黒勝ち',
    'result.draw': '½-½ ドロー',
    'analysis.title': 'エンジン解析',
    'analysis.start': '解析',
    'analysis.stop': '中止',
    'analysis.summary.best': '最善',
    'analysis.summary.excellent': '優秀',
    'analysis.summary.good': '良好',
    'analysis.summary.inaccuracies': '不正確',
    'analysis.summary.mistakes': 'ミス',
    'analysis.summary.blunders': '大悪手',
    'analysis.summary.book': '定跡',
    'lang.label': '言語',
  },

  pt: {
    'app.title': 'CheckAI — Xadrez para agentes de IA',
    'nav.dashboard': 'Painel',
    'nav.game': 'Jogo',
    'nav.archive': 'Arquivo',
    'game.resign': 'Desistir',
    'game.flip_board': 'Virar tabuleiro',
    'result.white_wins': '1-0 Brancas vencem',
    'result.black_wins': '0-1 Pretas vencem',
    'result.draw': '½-½ Empate',
    'analysis.title': 'Análise do motor',
    'analysis.start': 'Analisar',
    'analysis.stop': 'Parar',
    'analysis.summary.best': 'Melhores',
    'analysis.summary.excellent': 'Excelentes',
    'analysis.summary.good': 'Boas',
    'analysis.summary.inaccuracies': 'Imprecisões',
    'analysis.summary.mistakes': 'Erros',
    'analysis.summary.blunders': 'Blunders',
    'analysis.summary.book': 'Livro',
    'lang.label': 'Idioma',
  },

  ru: {
    'app.title': 'CheckAI — Шахматы для ИИ-агентов',
    'nav.dashboard': 'Панель',
    'nav.game': 'Игра',
    'nav.archive': 'Архив',
    'game.resign': 'Сдаться',
    'game.flip_board': 'Перевернуть доску',
    'result.white_wins': '1-0 Белые побеждают',
    'result.black_wins': '0-1 Чёрные побеждают',
    'result.draw': '½-½ Ничья',
    'analysis.title': 'Анализ движка',
    'analysis.start': 'Анализ',
    'analysis.stop': 'Стоп',
    'analysis.summary.best': 'Лучшие',
    'analysis.summary.excellent': 'Отличные',
    'analysis.summary.good': 'Хорошие',
    'analysis.summary.inaccuracies': 'Неточности',
    'analysis.summary.mistakes': 'Ошибки',
    'analysis.summary.blunders': 'Зевки',
    'analysis.summary.book': 'Дебют',
    'lang.label': 'Язык',
  },
};

export interface LocaleInfo {
  code: string;
  name: string;
}

export const SUPPORTED_LOCALES: LocaleInfo[] = [
  { code: 'en', name: 'English' },
  { code: 'de', name: 'Deutsch' },
  { code: 'fr', name: 'Français' },
  { code: 'es', name: 'Español' },
  { code: 'zh-CN', name: '中文' },
  { code: 'ja', name: '日本語' },
  { code: 'pt', name: 'Português' },
  { code: 'ru', name: 'Русский' },
];

let currentLocale = 'en';

/** Detect a suitable locale from the browser. */
function detectLocale(): string {
  const saved = localStorage.getItem('checkai-locale');
  if (saved && DICTIONARIES[saved]) return saved;

  const nav = navigator.language || 'en';
  if (DICTIONARIES[nav]) return nav;
  const base = nav.split('-')[0];
  if (DICTIONARIES[base]) return base;
  return 'en';
}

/** Translate a key with optional interpolation. */
export function t(key: string, params?: Record<string, string | number>): string {
  const dict = DICTIONARIES[currentLocale];
  let val = dict?.[key] ?? DICTIONARIES.en[key] ?? key;
  if (params) {
    for (const [k, v] of Object.entries(params)) {
      val = val.replace(`%{${k}}`, String(v));
    }
  }
  return val;
}

/** Get current locale code. */
export function getLocale(): string {
  return currentLocale;
}

/** Set locale and re-translate static DOM elements. */
export function setLocale(code: string): void {
  if (!DICTIONARIES[code]) return;
  currentLocale = code;
  localStorage.setItem('checkai-locale', code);
  translateDom();
}

/** Translate all static `[data-i18n]` elements in the DOM. */
export function translateDom(): void {
  document.querySelectorAll<HTMLElement>('[data-i18n]').forEach((el) => {
    const key = el.dataset.i18n!;
    el.textContent = t(key);
  });
  document.querySelectorAll<HTMLInputElement>('[data-i18n-placeholder]').forEach((el) => {
    const key = el.dataset.i18nPlaceholder!;
    el.placeholder = t(key);
  });
  document.querySelectorAll<HTMLElement>('[data-i18n-title]').forEach((el) => {
    const key = el.dataset.i18nTitle!;
    el.title = t(key);
  });
  document.title = t('app.title');
}

/** Initialize the i18n system. */
export function initI18n(): void {
  currentLocale = detectLocale();

  // Populate language dropdown
  const select = document.getElementById('lang-select') as HTMLSelectElement | null;
  if (select) {
    select.innerHTML = '';
    for (const loc of SUPPORTED_LOCALES) {
      const opt = document.createElement('option');
      opt.value = loc.code;
      opt.textContent = loc.name;
      select.appendChild(opt);
    }
    select.value = currentLocale;
  }

  translateDom();
}
