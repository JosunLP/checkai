import { defineConfig } from 'vitepress';

export default defineConfig({
  title: 'CheckAI',
  description:
    'Chess Server for AI Agents — REST & WebSocket API with FIDE 2023 Rules',

  // Deploy to GitHub Pages under /checkai/
  base: '/checkai/',

  head: [
    ['link', { rel: 'icon', type: 'image/svg+xml', href: '/checkai/logo.svg' }],
    ['meta', { name: 'theme-color', content: '#10b981' }],
    [
      'meta',
      {
        name: 'og:description',
        content:
          'A Rust chess server and CLI for AI agents with REST, WebSocket, and deep analysis APIs.',
      },
    ],
  ],

  lastUpdated: true,
  cleanUrls: true,

  themeConfig: {
    logo: '/logo.svg',

    nav: [
      { text: 'Guide', link: '/guide/getting-started' },
      { text: 'API Reference', link: '/api/rest' },
      { text: 'Agent Protocol', link: '/agent/overview' },
      {
        text: 'v0.3.1',
        items: [
          {
            text: 'Changelog',
            link: '/changelog',
          },
          {
            text: 'Releases',
            link: 'https://github.com/JosunLP/checkai/releases',
          },
        ],
      },
    ],

    sidebar: {
      '/guide/': [
        {
          text: 'Introduction',
          items: [
            { text: 'What is CheckAI?', link: '/guide/what-is-checkai' },
            { text: 'Getting Started', link: '/guide/getting-started' },
          ],
        },
        {
          text: 'Usage',
          items: [
            { text: 'CLI Commands', link: '/guide/cli' },
            { text: 'Docker', link: '/guide/docker' },
            { text: 'Configuration', link: '/guide/configuration' },
            { text: 'Web UI', link: '/guide/web-ui' },
          ],
        },
        {
          text: 'Engine',
          items: [
            { text: 'Analysis Engine', link: '/guide/analysis' },
            { text: 'Opening Book', link: '/guide/opening-book' },
            { text: 'Endgame Tablebases', link: '/guide/tablebases' },
          ],
        },
        {
          text: 'Development',
          items: [
            { text: 'Architecture', link: '/guide/architecture' },
            { text: 'Internationalization', link: '/guide/i18n' },
          ],
        },
      ],
      '/api/': [
        {
          text: 'API Reference',
          items: [
            { text: 'REST API', link: '/api/rest' },
            { text: 'WebSocket API', link: '/api/websocket' },
            { text: 'Analysis API', link: '/api/analysis' },
          ],
        },
      ],
      '/agent/': [
        {
          text: 'Agent Protocol',
          items: [
            { text: 'Overview', link: '/agent/overview' },
            { text: 'Game State (Input)', link: '/agent/game-state' },
            { text: 'Move Output', link: '/agent/move-output' },
            { text: 'Chess Rules', link: '/agent/chess-rules' },
            { text: 'Special Actions', link: '/agent/special-actions' },
            { text: 'Examples', link: '/agent/examples' },
          ],
        },
      ],
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/JosunLP/checkai' },
    ],

    search: {
      provider: 'local',
    },

    editLink: {
      pattern: 'https://github.com/JosunLP/checkai/edit/main/docs/:path',
      text: 'Edit this page on GitHub',
    },

    footer: {
      message: 'Released under the MIT License.',
      copyright: 'Copyright © 2025-present JosunLP',
    },
  },
});
