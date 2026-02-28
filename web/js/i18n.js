/**
 * CheckAI Web UI — Internationalization (i18n)
 *
 * Supports: en, de, fr, es, zh-CN, ja, pt, ru
 * Default / fallback: en
 *
 * Usage:
 *   t('key')              — translate a simple key
 *   t('key', { n: 42 })   — translate with interpolation (%{n})
 *   setLocale('de')        — switch language at runtime
 *   getLocale()            — current locale string
 */

// ============================================================================
// Translation dictionaries
// ============================================================================

const I18N = {
  // ---------------------------------------------------------------------------
  // English (default / fallback)
  // ---------------------------------------------------------------------------
  en: {
    // Meta
    'app.title': 'CheckAI — Chess for AI Agents',

    // Nav
    'nav.dashboard': 'Dashboard',
    'nav.game': 'Game',
    'nav.archive': 'Archive',

    // WebSocket status
    'ws.connected': 'Connected',
    'ws.disconnected': 'Disconnected',

    // Dashboard
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

    // Game view
    'game.no_game_selected': 'No game selected.',
    'game.no_game_hint':
      'Select a game from the dashboard or create a new one.',
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

    // Game dynamic
    'game.turn_white': '♔ White',
    'game.turn_black': '♚ Black',
    'game.status_over': 'Finished',
    'game.status_active': 'In Progress',
    'game.check_yes': '⚠ Yes!',
    'game.check_no': 'No',
    'game.your_turn': '⏱ Your turn',
    'game.badge_over': 'Finished',
    'game.badge_active': 'Active',

    // Results
    'result.white_wins': '1-0 White wins',
    'result.black_wins': '0-1 Black wins',
    'result.draw': '½-½ Draw',

    // End reasons
    'reason.Checkmate': 'Checkmate',
    'reason.Stalemate': 'Stalemate',
    'reason.ThreefoldRepetition': 'Threefold Repetition',
    'reason.FivefoldRepetition': 'Fivefold Repetition',
    'reason.FiftyMoveRule': 'Fifty-Move Rule',
    'reason.SeventyFiveMoveRule': 'Seventy-Five-Move Rule',
    'reason.InsufficientMaterial': 'Insufficient Material',
    'reason.Resignation': 'Resignation',
    'reason.DrawAgreement': 'Draw by Agreement',

    // Piece names
    'piece.K': 'King',
    'piece.Q': 'Queen',
    'piece.R': 'Rook',
    'piece.B': 'Bishop',
    'piece.N': 'Knight',
    'piece.P': 'Pawn',

    // Archive
    'archive.title': 'Game Archive',
    'archive.refresh': '↻ Refresh',
    'archive.no_games': 'No archived games yet.',
    'archive.badge': 'Archive',
    'archive.half_moves': '%{n} half-moves',
    'archive.replay': 'Replay',
    'archive.replay_title': 'Replay — %{id}',
    'archive.close': '✕ Close',
    'archive.move_label': 'Move',

    // Toasts & messages
    'toast.new_game_created': 'New game created!',
    'toast.game_deleted': 'Game deleted',
    'toast.current_game_deleted': 'Current game was deleted',
    'toast.invalid_move': 'Invalid move: %{error}',
    'toast.error': 'Error: %{error}',
    'toast.load_archive_failed': 'Failed to load archive: %{error}',
    'toast.replay_failed': 'Replay failed: %{error}',

    // Confirmations
    'confirm.resign': 'Really resign?',
    'confirm.delete': 'Really delete this game?',
    'confirm.claim_draw_reason':
      'Enter reason: threefold_repetition or fifty_move_rule',

    // Move input
    'toast.enter_from_to': 'Please enter "from" and "to" squares',

    // Language selector
    'lang.label': 'Language',
  },

  // ---------------------------------------------------------------------------
  // German
  // ---------------------------------------------------------------------------
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
    'game.no_game_hint':
      'Wähle ein Spiel aus dem Dashboard oder erstelle ein neues.',
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
    'confirm.resign': 'Wirklich aufgeben?',
    'confirm.delete': 'Dieses Spiel wirklich löschen?',
    'confirm.claim_draw_reason':
      'Grund angeben: threefold_repetition oder fifty_move_rule',
    'toast.enter_from_to': 'Bitte "von" und "nach" Feld angeben',
    'lang.label': 'Sprache',
  },

  // ---------------------------------------------------------------------------
  // French
  // ---------------------------------------------------------------------------
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
    'dashboard.refresh': 'Actualiser',
    'stat.active_games': 'Parties actives',
    'stat.archived_games': 'Parties archivées',
    'stat.active_storage': 'Stockage actif',
    'stat.archive_storage': 'Stockage archives',
    'game.no_game_selected': 'Aucune partie sélectionnée.',
    'game.no_game_hint': 'Sélectionnez une partie ou créez-en une nouvelle.',
    'game.black': 'Noir',
    'game.white': 'Blanc',
    'game.promotion_title': 'Promotion du pion',
    'game.info': 'Infos partie',
    'game.game_id': 'ID partie',
    'game.turn': 'Au trait',
    'game.move_number': 'N° de coup',
    'game.status': 'Statut',
    'game.check': 'Échec',
    'game.legal_moves': 'Coups légaux',
    'game.actions': 'Actions',
    'game.resign': 'Abandonner',
    'game.offer_draw': 'Proposer nulle',
    'game.claim_draw': 'Réclamer nulle',
    'game.delete': 'Supprimer',
    'game.move_input': 'Entrer un coup',
    'game.from_placeholder': 'de (ex. e2)',
    'game.to_placeholder': 'vers (ex. e4)',
    'game.submit_move': 'Jouer',
    'game.move_history': 'Historique',
    'game.no_moves': 'Aucun coup joué.',
    'game.castling_rights': 'Droits de roque',
    'game.kingside': 'Petit roque',
    'game.queenside': 'Grand roque',
    'game.turn_white': '♔ Blanc',
    'game.turn_black': '♚ Noir',
    'game.status_over': 'Terminée',
    'game.status_active': 'En cours',
    'game.check_yes': '⚠ Oui !',
    'game.check_no': 'Non',
    'game.your_turn': '⏱ Au trait',
    'game.badge_over': 'Terminée',
    'game.badge_active': 'Active',
    'result.white_wins': '1-0 Les blancs gagnent',
    'result.black_wins': '0-1 Les noirs gagnent',
    'result.draw': '½-½ Nulle',
    'reason.Checkmate': 'Échec et mat',
    'reason.Stalemate': 'Pat',
    'reason.ThreefoldRepetition': 'Triple répétition',
    'reason.FivefoldRepetition': 'Quintuple répétition',
    'reason.FiftyMoveRule': 'Règle des 50 coups',
    'reason.SeventyFiveMoveRule': 'Règle des 75 coups',
    'reason.InsufficientMaterial': 'Matériel insuffisant',
    'reason.Resignation': 'Abandon',
    'reason.DrawAgreement': 'Nulle par accord',
    'piece.K': 'Roi',
    'piece.Q': 'Dame',
    'piece.R': 'Tour',
    'piece.B': 'Fou',
    'piece.N': 'Cavalier',
    'piece.P': 'Pion',
    'archive.title': 'Archives des parties',
    'archive.refresh': '↻ Actualiser',
    'archive.no_games': 'Aucune partie archivée.',
    'archive.badge': 'Archive',
    'archive.half_moves': '%{n} demi-coups',
    'archive.replay': 'Rejouer',
    'archive.replay_title': 'Rejouer — %{id}',
    'archive.close': '✕ Fermer',
    'archive.move_label': 'Coup',
    'toast.new_game_created': 'Nouvelle partie créée !',
    'toast.game_deleted': 'Partie supprimée',
    'toast.current_game_deleted': 'La partie en cours a été supprimée',
    'toast.invalid_move': 'Coup invalide : %{error}',
    'toast.error': 'Erreur : %{error}',
    'toast.load_archive_failed': 'Échec du chargement des archives : %{error}',
    'toast.replay_failed': 'Échec de la relecture : %{error}',
    'confirm.resign': 'Vraiment abandonner ?',
    'confirm.delete': 'Vraiment supprimer cette partie ?',
    'confirm.claim_draw_reason':
      'Indiquez la raison : threefold_repetition ou fifty_move_rule',
    'toast.enter_from_to': 'Veuillez entrer les cases "de" et "vers"',
    'lang.label': 'Langue',
  },

  // ---------------------------------------------------------------------------
  // Spanish
  // ---------------------------------------------------------------------------
  es: {
    'app.title': 'CheckAI — Ajedrez para agentes de IA',
    'nav.dashboard': 'Panel',
    'nav.game': 'Partida',
    'nav.archive': 'Archivo',
    'ws.connected': 'Conectado',
    'ws.disconnected': 'Desconectado',
    'dashboard.active_games': 'Partidas activas',
    'dashboard.new_game': 'Nueva partida',
    'dashboard.no_games': 'Aún no hay partidas.',
    'dashboard.no_games_hint': 'Crea una nueva partida para comenzar.',
    'dashboard.storage_stats': 'Estadísticas de almacenamiento',
    'dashboard.refresh': 'Actualizar',
    'stat.active_games': 'Partidas activas',
    'stat.archived_games': 'Partidas archivadas',
    'stat.active_storage': 'Almacenamiento activo',
    'stat.archive_storage': 'Almacenamiento archivo',
    'game.no_game_selected': 'No hay partida seleccionada.',
    'game.no_game_hint': 'Selecciona una partida del panel o crea una nueva.',
    'game.black': 'Negras',
    'game.white': 'Blancas',
    'game.promotion_title': 'Promoción de peón',
    'game.info': 'Información',
    'game.game_id': 'ID de partida',
    'game.turn': 'Turno',
    'game.move_number': 'Nº de jugada',
    'game.status': 'Estado',
    'game.check': 'Jaque',
    'game.legal_moves': 'Jugadas legales',
    'game.actions': 'Acciones',
    'game.resign': 'Rendirse',
    'game.offer_draw': 'Ofrecer tablas',
    'game.claim_draw': 'Reclamar tablas',
    'game.delete': 'Eliminar partida',
    'game.move_input': 'Introducir jugada',
    'game.from_placeholder': 'desde (ej. e2)',
    'game.to_placeholder': 'hasta (ej. e4)',
    'game.submit_move': 'Mover',
    'game.move_history': 'Historial',
    'game.no_moves': 'Aún no hay jugadas.',
    'game.castling_rights': 'Derechos de enroque',
    'game.kingside': 'Flanco de rey',
    'game.queenside': 'Flanco de dama',
    'game.turn_white': '♔ Blancas',
    'game.turn_black': '♚ Negras',
    'game.status_over': 'Finalizada',
    'game.status_active': 'En curso',
    'game.check_yes': '⚠ ¡Sí!',
    'game.check_no': 'No',
    'game.your_turn': '⏱ Tu turno',
    'game.badge_over': 'Finalizada',
    'game.badge_active': 'Activa',
    'result.white_wins': '1-0 Ganan blancas',
    'result.black_wins': '0-1 Ganan negras',
    'result.draw': '½-½ Tablas',
    'reason.Checkmate': 'Jaque mate',
    'reason.Stalemate': 'Ahogado',
    'reason.ThreefoldRepetition': 'Triple repetición',
    'reason.FivefoldRepetition': 'Quíntuple repetición',
    'reason.FiftyMoveRule': 'Regla de los 50 movimientos',
    'reason.SeventyFiveMoveRule': 'Regla de los 75 movimientos',
    'reason.InsufficientMaterial': 'Material insuficiente',
    'reason.Resignation': 'Rendición',
    'reason.DrawAgreement': 'Tablas por acuerdo',
    'piece.K': 'Rey',
    'piece.Q': 'Dama',
    'piece.R': 'Torre',
    'piece.B': 'Alfil',
    'piece.N': 'Caballo',
    'piece.P': 'Peón',
    'archive.title': 'Archivo de partidas',
    'archive.refresh': '↻ Actualizar',
    'archive.no_games': 'Aún no hay partidas archivadas.',
    'archive.badge': 'Archivo',
    'archive.half_moves': '%{n} semijugadas',
    'archive.replay': 'Repetir',
    'archive.replay_title': 'Repetición — %{id}',
    'archive.close': '✕ Cerrar',
    'archive.move_label': 'Jugada',
    'toast.new_game_created': '¡Nueva partida creada!',
    'toast.game_deleted': 'Partida eliminada',
    'toast.current_game_deleted': 'La partida actual fue eliminada',
    'toast.invalid_move': 'Jugada inválida: %{error}',
    'toast.error': 'Error: %{error}',
    'toast.load_archive_failed': 'Error al cargar el archivo: %{error}',
    'toast.replay_failed': 'Error en la repetición: %{error}',
    'confirm.resign': '¿Realmente rendirse?',
    'confirm.delete': '¿Realmente eliminar esta partida?',
    'confirm.claim_draw_reason':
      'Indicar razón: threefold_repetition o fifty_move_rule',
    'toast.enter_from_to': 'Por favor ingrese las casillas "desde" y "hasta"',
    'lang.label': 'Idioma',
  },

  // ---------------------------------------------------------------------------
  // Chinese (Simplified)
  // ---------------------------------------------------------------------------
  'zh-CN': {
    'app.title': 'CheckAI — AI代理国际象棋',
    'nav.dashboard': '仪表盘',
    'nav.game': '对局',
    'nav.archive': '存档',
    'ws.connected': '已连接',
    'ws.disconnected': '已断开',
    'dashboard.active_games': '活跃对局',
    'dashboard.new_game': '新建对局',
    'dashboard.no_games': '暂无对局。',
    'dashboard.no_games_hint': '创建新对局以开始。',
    'dashboard.storage_stats': '存储统计',
    'dashboard.refresh': '刷新',
    'stat.active_games': '活跃对局',
    'stat.archived_games': '已归档对局',
    'stat.active_storage': '活跃存储',
    'stat.archive_storage': '归档存储',
    'game.no_game_selected': '未选择对局。',
    'game.no_game_hint': '从仪表盘选择对局或创建新对局。',
    'game.black': '黑方',
    'game.white': '白方',
    'game.promotion_title': '兵的升变',
    'game.info': '对局信息',
    'game.game_id': '对局 ID',
    'game.turn': '当前走棋',
    'game.move_number': '回合数',
    'game.status': '状态',
    'game.check': '将军',
    'game.legal_moves': '合法走法',
    'game.actions': '操作',
    'game.resign': '认输',
    'game.offer_draw': '提议和棋',
    'game.claim_draw': '申请和棋',
    'game.delete': '删除对局',
    'game.move_input': '输入走法',
    'game.from_placeholder': '从（如 e2）',
    'game.to_placeholder': '到（如 e4）',
    'game.submit_move': '走棋',
    'game.move_history': '走法记录',
    'game.no_moves': '暂无走法。',
    'game.castling_rights': '王车易位权',
    'game.kingside': '王翼',
    'game.queenside': '后翼',
    'game.turn_white': '♔ 白方',
    'game.turn_black': '♚ 黑方',
    'game.status_over': '已结束',
    'game.status_active': '进行中',
    'game.check_yes': '⚠ 是！',
    'game.check_no': '否',
    'game.your_turn': '⏱ 走棋中',
    'game.badge_over': '已结束',
    'game.badge_active': '进行中',
    'result.white_wins': '1-0 白方胜',
    'result.black_wins': '0-1 黑方胜',
    'result.draw': '½-½ 和棋',
    'reason.Checkmate': '将杀',
    'reason.Stalemate': '逼和',
    'reason.ThreefoldRepetition': '三次重复局面',
    'reason.FivefoldRepetition': '五次重复局面',
    'reason.FiftyMoveRule': '50回合规则',
    'reason.SeventyFiveMoveRule': '75回合规则',
    'reason.InsufficientMaterial': '子力不足',
    'reason.Resignation': '认输',
    'reason.DrawAgreement': '协议和棋',
    'piece.K': '王',
    'piece.Q': '后',
    'piece.R': '车',
    'piece.B': '象',
    'piece.N': '马',
    'piece.P': '兵',
    'archive.title': '对局存档',
    'archive.refresh': '↻ 刷新',
    'archive.no_games': '暂无归档对局。',
    'archive.badge': '存档',
    'archive.half_moves': '%{n} 半回合',
    'archive.replay': '回放',
    'archive.replay_title': '回放 — %{id}',
    'archive.close': '✕ 关闭',
    'archive.move_label': '回合',
    'toast.new_game_created': '新对局已创建！',
    'toast.game_deleted': '对局已删除',
    'toast.current_game_deleted': '当前对局已被删除',
    'toast.invalid_move': '非法走法：%{error}',
    'toast.error': '错误：%{error}',
    'toast.load_archive_failed': '加载存档失败：%{error}',
    'toast.replay_failed': '回放失败：%{error}',
    'confirm.resign': '确认认输？',
    'confirm.delete': '确认删除此对局？',
    'confirm.claim_draw_reason':
      '请输入原因：threefold_repetition 或 fifty_move_rule',
    'toast.enter_from_to': '请输入"从"和"到"格子',
    'lang.label': '语言',
  },

  // ---------------------------------------------------------------------------
  // Japanese
  // ---------------------------------------------------------------------------
  ja: {
    'app.title': 'CheckAI — AIエージェント用チェス',
    'nav.dashboard': 'ダッシュボード',
    'nav.game': '対局',
    'nav.archive': 'アーカイブ',
    'ws.connected': '接続済み',
    'ws.disconnected': '切断',
    'dashboard.active_games': 'アクティブな対局',
    'dashboard.new_game': '新規対局',
    'dashboard.no_games': 'まだ対局がありません。',
    'dashboard.no_games_hint': '新しい対局を作成してください。',
    'dashboard.storage_stats': 'ストレージ統計',
    'dashboard.refresh': '更新',
    'stat.active_games': 'アクティブ対局',
    'stat.archived_games': 'アーカイブ対局',
    'stat.active_storage': 'アクティブストレージ',
    'stat.archive_storage': 'アーカイブストレージ',
    'game.no_game_selected': '対局が選択されていません。',
    'game.no_game_hint':
      'ダッシュボードから対局を選択するか、新規作成してください。',
    'game.black': '黒',
    'game.white': '白',
    'game.promotion_title': 'ポーンのプロモーション',
    'game.info': '対局情報',
    'game.game_id': '対局ID',
    'game.turn': '手番',
    'game.move_number': '手数',
    'game.status': 'ステータス',
    'game.check': 'チェック',
    'game.legal_moves': '合法手',
    'game.actions': 'アクション',
    'game.resign': '投了',
    'game.offer_draw': 'ドロー提案',
    'game.claim_draw': 'ドロー請求',
    'game.delete': '対局削除',
    'game.move_input': '手を入力',
    'game.from_placeholder': 'from（例：e2）',
    'game.to_placeholder': 'to（例：e4）',
    'game.submit_move': '指す',
    'game.move_history': '棋譜',
    'game.no_moves': 'まだ手がありません。',
    'game.castling_rights': 'キャスリング権',
    'game.kingside': 'キングサイド',
    'game.queenside': 'クイーンサイド',
    'game.turn_white': '♔ 白',
    'game.turn_black': '♚ 黒',
    'game.status_over': '終了',
    'game.status_active': '進行中',
    'game.check_yes': '⚠ はい！',
    'game.check_no': 'いいえ',
    'game.your_turn': '⏱ 手番',
    'game.badge_over': '終了',
    'game.badge_active': 'アクティブ',
    'result.white_wins': '1-0 白勝ち',
    'result.black_wins': '0-1 黒勝ち',
    'result.draw': '½-½ ドロー',
    'reason.Checkmate': 'チェックメイト',
    'reason.Stalemate': 'ステイルメイト',
    'reason.ThreefoldRepetition': '三回同一局面',
    'reason.FivefoldRepetition': '五回同一局面',
    'reason.FiftyMoveRule': '50手ルール',
    'reason.SeventyFiveMoveRule': '75手ルール',
    'reason.InsufficientMaterial': '持駒不足',
    'reason.Resignation': '投了',
    'reason.DrawAgreement': '合意ドロー',
    'piece.K': 'キング',
    'piece.Q': 'クイーン',
    'piece.R': 'ルーク',
    'piece.B': 'ビショップ',
    'piece.N': 'ナイト',
    'piece.P': 'ポーン',
    'archive.title': '対局アーカイブ',
    'archive.refresh': '↻ 更新',
    'archive.no_games': 'アーカイブされた対局はありません。',
    'archive.badge': 'アーカイブ',
    'archive.half_moves': '%{n} 半手',
    'archive.replay': 'リプレイ',
    'archive.replay_title': 'リプレイ — %{id}',
    'archive.close': '✕ 閉じる',
    'archive.move_label': '手',
    'toast.new_game_created': '新規対局を作成しました！',
    'toast.game_deleted': '対局を削除しました',
    'toast.current_game_deleted': '現在の対局が削除されました',
    'toast.invalid_move': '不正な手：%{error}',
    'toast.error': 'エラー：%{error}',
    'toast.load_archive_failed': 'アーカイブの読み込みに失敗：%{error}',
    'toast.replay_failed': 'リプレイに失敗：%{error}',
    'confirm.resign': '本当に投了しますか？',
    'confirm.delete': '本当にこの対局を削除しますか？',
    'confirm.claim_draw_reason':
      '理由を入力：threefold_repetition または fifty_move_rule',
    'toast.enter_from_to': '"from"と"to"のマスを入力してください',
    'lang.label': '言語',
  },

  // ---------------------------------------------------------------------------
  // Portuguese
  // ---------------------------------------------------------------------------
  pt: {
    'app.title': 'CheckAI — Xadrez para agentes de IA',
    'nav.dashboard': 'Painel',
    'nav.game': 'Jogo',
    'nav.archive': 'Arquivo',
    'ws.connected': 'Conectado',
    'ws.disconnected': 'Desconectado',
    'dashboard.active_games': 'Jogos ativos',
    'dashboard.new_game': 'Novo jogo',
    'dashboard.no_games': 'Nenhum jogo ainda.',
    'dashboard.no_games_hint': 'Crie um novo jogo para começar.',
    'dashboard.storage_stats': 'Estatísticas de armazenamento',
    'dashboard.refresh': 'Atualizar',
    'stat.active_games': 'Jogos ativos',
    'stat.archived_games': 'Jogos arquivados',
    'stat.active_storage': 'Armazenamento ativo',
    'stat.archive_storage': 'Armazenamento arquivo',
    'game.no_game_selected': 'Nenhum jogo selecionado.',
    'game.no_game_hint': 'Selecione um jogo do painel ou crie um novo.',
    'game.black': 'Pretas',
    'game.white': 'Brancas',
    'game.promotion_title': 'Promoção de peão',
    'game.info': 'Info do jogo',
    'game.game_id': 'ID do jogo',
    'game.turn': 'Vez de',
    'game.move_number': 'Nº do lance',
    'game.status': 'Estado',
    'game.check': 'Xeque',
    'game.legal_moves': 'Lances legais',
    'game.actions': 'Ações',
    'game.resign': 'Desistir',
    'game.offer_draw': 'Oferecer empate',
    'game.claim_draw': 'Reivindicar empate',
    'game.delete': 'Excluir jogo',
    'game.move_input': 'Inserir lance',
    'game.from_placeholder': 'de (ex. e2)',
    'game.to_placeholder': 'para (ex. e4)',
    'game.submit_move': 'Jogar',
    'game.move_history': 'Histórico',
    'game.no_moves': 'Nenhum lance ainda.',
    'game.castling_rights': 'Direitos de roque',
    'game.kingside': 'Lado do rei',
    'game.queenside': 'Lado da dama',
    'game.turn_white': '♔ Brancas',
    'game.turn_black': '♚ Pretas',
    'game.status_over': 'Encerrado',
    'game.status_active': 'Em andamento',
    'game.check_yes': '⚠ Sim!',
    'game.check_no': 'Não',
    'game.your_turn': '⏱ Sua vez',
    'game.badge_over': 'Encerrado',
    'game.badge_active': 'Ativo',
    'result.white_wins': '1-0 Brancas vencem',
    'result.black_wins': '0-1 Pretas vencem',
    'result.draw': '½-½ Empate',
    'reason.Checkmate': 'Xeque-mate',
    'reason.Stalemate': 'Afogamento',
    'reason.ThreefoldRepetition': 'Tripla repetição',
    'reason.FivefoldRepetition': 'Quíntupla repetição',
    'reason.FiftyMoveRule': 'Regra dos 50 lances',
    'reason.SeventyFiveMoveRule': 'Regra dos 75 lances',
    'reason.InsufficientMaterial': 'Material insuficiente',
    'reason.Resignation': 'Desistência',
    'reason.DrawAgreement': 'Empate por acordo',
    'piece.K': 'Rei',
    'piece.Q': 'Dama',
    'piece.R': 'Torre',
    'piece.B': 'Bispo',
    'piece.N': 'Cavalo',
    'piece.P': 'Peão',
    'archive.title': 'Arquivo de jogos',
    'archive.refresh': '↻ Atualizar',
    'archive.no_games': 'Nenhum jogo arquivado.',
    'archive.badge': 'Arquivo',
    'archive.half_moves': '%{n} meio-lances',
    'archive.replay': 'Repetir',
    'archive.replay_title': 'Repetição — %{id}',
    'archive.close': '✕ Fechar',
    'archive.move_label': 'Lance',
    'toast.new_game_created': 'Novo jogo criado!',
    'toast.game_deleted': 'Jogo excluído',
    'toast.current_game_deleted': 'O jogo atual foi excluído',
    'toast.invalid_move': 'Lance inválido: %{error}',
    'toast.error': 'Erro: %{error}',
    'toast.load_archive_failed': 'Falha ao carregar arquivo: %{error}',
    'toast.replay_failed': 'Falha na repetição: %{error}',
    'confirm.resign': 'Realmente desistir?',
    'confirm.delete': 'Realmente excluir este jogo?',
    'confirm.claim_draw_reason':
      'Informe o motivo: threefold_repetition ou fifty_move_rule',
    'toast.enter_from_to': 'Por favor insira as casas "de" e "para"',
    'lang.label': 'Idioma',
  },

  // ---------------------------------------------------------------------------
  // Russian
  // ---------------------------------------------------------------------------
  ru: {
    'app.title': 'CheckAI — Шахматы для ИИ-агентов',
    'nav.dashboard': 'Панель',
    'nav.game': 'Игра',
    'nav.archive': 'Архив',
    'ws.connected': 'Подключено',
    'ws.disconnected': 'Отключено',
    'dashboard.active_games': 'Активные партии',
    'dashboard.new_game': 'Новая партия',
    'dashboard.no_games': 'Пока нет партий.',
    'dashboard.no_games_hint': 'Создайте новую партию для начала.',
    'dashboard.storage_stats': 'Статистика хранилища',
    'dashboard.refresh': 'Обновить',
    'stat.active_games': 'Активные партии',
    'stat.archived_games': 'Архивные партии',
    'stat.active_storage': 'Активное хранилище',
    'stat.archive_storage': 'Архивное хранилище',
    'game.no_game_selected': 'Партия не выбрана.',
    'game.no_game_hint': 'Выберите партию из панели или создайте новую.',
    'game.black': 'Чёрные',
    'game.white': 'Белые',
    'game.promotion_title': 'Превращение пешки',
    'game.info': 'Информация',
    'game.game_id': 'ID партии',
    'game.turn': 'Ход',
    'game.move_number': 'Номер хода',
    'game.status': 'Статус',
    'game.check': 'Шах',
    'game.legal_moves': 'Легальные ходы',
    'game.actions': 'Действия',
    'game.resign': 'Сдаться',
    'game.offer_draw': 'Предложить ничью',
    'game.claim_draw': 'Потребовать ничью',
    'game.delete': 'Удалить партию',
    'game.move_input': 'Ввести ход',
    'game.from_placeholder': 'откуда (напр. e2)',
    'game.to_placeholder': 'куда (напр. e4)',
    'game.submit_move': 'Ходить',
    'game.move_history': 'История ходов',
    'game.no_moves': 'Пока нет ходов.',
    'game.castling_rights': 'Права рокировки',
    'game.kingside': 'Королевский фланг',
    'game.queenside': 'Ферзевый фланг',
    'game.turn_white': '♔ Белые',
    'game.turn_black': '♚ Чёрные',
    'game.status_over': 'Завершена',
    'game.status_active': 'Идёт',
    'game.check_yes': '⚠ Да!',
    'game.check_no': 'Нет',
    'game.your_turn': '⏱ Ваш ход',
    'game.badge_over': 'Завершена',
    'game.badge_active': 'Активна',
    'result.white_wins': '1-0 Белые побеждают',
    'result.black_wins': '0-1 Чёрные побеждают',
    'result.draw': '½-½ Ничья',
    'reason.Checkmate': 'Мат',
    'reason.Stalemate': 'Пат',
    'reason.ThreefoldRepetition': 'Троекратное повторение',
    'reason.FivefoldRepetition': 'Пятикратное повторение',
    'reason.FiftyMoveRule': 'Правило 50 ходов',
    'reason.SeventyFiveMoveRule': 'Правило 75 ходов',
    'reason.InsufficientMaterial': 'Недостаточно материала',
    'reason.Resignation': 'Сдача',
    'reason.DrawAgreement': 'Ничья по соглашению',
    'piece.K': 'Король',
    'piece.Q': 'Ферзь',
    'piece.R': 'Ладья',
    'piece.B': 'Слон',
    'piece.N': 'Конь',
    'piece.P': 'Пешка',
    'archive.title': 'Архив партий',
    'archive.refresh': '↻ Обновить',
    'archive.no_games': 'Архивных партий нет.',
    'archive.badge': 'Архив',
    'archive.half_moves': '%{n} полуходов',
    'archive.replay': 'Повтор',
    'archive.replay_title': 'Повтор — %{id}',
    'archive.close': '✕ Закрыть',
    'archive.move_label': 'Ход',
    'toast.new_game_created': 'Новая партия создана!',
    'toast.game_deleted': 'Партия удалена',
    'toast.current_game_deleted': 'Текущая партия была удалена',
    'toast.invalid_move': 'Недопустимый ход: %{error}',
    'toast.error': 'Ошибка: %{error}',
    'toast.load_archive_failed': 'Не удалось загрузить архив: %{error}',
    'toast.replay_failed': 'Не удалось воспроизвести: %{error}',
    'confirm.resign': 'Действительно сдаться?',
    'confirm.delete': 'Действительно удалить эту партию?',
    'confirm.claim_draw_reason':
      'Укажите причину: threefold_repetition или fifty_move_rule',
    'toast.enter_from_to': 'Пожалуйста, укажите поля "откуда" и "куда"',
    'lang.label': 'Язык',
  },
};

// ============================================================================
// Supported locales list (for UI language picker)
// ============================================================================

const SUPPORTED_LOCALES = [
  { code: 'en', name: 'English' },
  { code: 'de', name: 'Deutsch' },
  { code: 'fr', name: 'Français' },
  { code: 'es', name: 'Español' },
  { code: 'zh-CN', name: '中文' },
  { code: 'ja', name: '日本語' },
  { code: 'pt', name: 'Português' },
  { code: 'ru', name: 'Русский' },
];

// ============================================================================
// Internal state
// ============================================================================

let _currentLocale = 'en';

// ============================================================================
// Public API
// ============================================================================

/**
 * Detects the best locale from the browser or stored preference.
 * Priority: localStorage > navigator.language > 'en'
 */
function detectLocale() {
  // 1. Stored preference
  const stored = localStorage.getItem('checkai-locale');
  if (stored && I18N[stored]) return stored;

  // 2. Browser language
  const nav = navigator.language || navigator.userLanguage || '';
  const normalized = normalizeLocale(nav);
  if (normalized) return normalized;

  // 3. Fallback
  return 'en';
}

/**
 * Normalizes a browser locale string to one of our supported locales.
 */
function normalizeLocale(tag) {
  const lower = tag.toLowerCase().replace('_', '-');
  if (lower.startsWith('zh')) return 'zh-CN';
  if (lower.startsWith('ja')) return 'ja';
  if (lower.startsWith('de')) return 'de';
  if (lower.startsWith('fr')) return 'fr';
  if (lower.startsWith('es')) return 'es';
  if (lower.startsWith('pt')) return 'pt';
  if (lower.startsWith('ru')) return 'ru';
  if (lower.startsWith('en')) return 'en';
  return null;
}

/**
 * Returns the current locale code.
 */
function getLocale() {
  return _currentLocale;
}

/**
 * Sets the active locale.
 * Persists the choice and re-translates all `[data-i18n]` elements in the DOM.
 */
function setLocale(locale) {
  if (!I18N[locale]) locale = 'en';
  _currentLocale = locale;
  localStorage.setItem('checkai-locale', locale);
  document.documentElement.lang = locale;

  // Update page title
  document.title = t('app.title');

  // Translate all static elements with data-i18n attributes
  document.querySelectorAll('[data-i18n]').forEach((el) => {
    const key = el.getAttribute('data-i18n');
    if (key) el.textContent = t(key);
  });

  // Translate all elements with data-i18n-placeholder
  document.querySelectorAll('[data-i18n-placeholder]').forEach((el) => {
    const key = el.getAttribute('data-i18n-placeholder');
    if (key) el.placeholder = t(key);
  });

  // Translate all elements with data-i18n-title
  document.querySelectorAll('[data-i18n-title]').forEach((el) => {
    const key = el.getAttribute('data-i18n-title');
    if (key) el.title = t(key);
  });

  // Update the language selector display
  const langSelect = document.getElementById('lang-select');
  if (langSelect) langSelect.value = locale;
}

/**
 * Translates a key, with optional interpolation.
 *
 * @param {string} key   — dot-separated translation key
 * @param {Object} [vars] — interpolation variables, replacing `%{name}`
 * @returns {string}
 */
function t(key, vars) {
  let str =
    (I18N[_currentLocale] && I18N[_currentLocale][key]) ||
    (I18N.en && I18N.en[key]) ||
    key;
  if (vars) {
    for (const [k, v] of Object.entries(vars)) {
      str = str.replace(new RegExp(`%\\{${k}\\}`, 'g'), String(v));
    }
  }
  return str;
}

/**
 * Initializes i18n: detects locale and applies translations.
 * Call this before other UI initialization.
 */
function initI18n() {
  _currentLocale = detectLocale();
  setLocale(_currentLocale);
}
