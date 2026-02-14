import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  integrations: [
    starlight({
      title: 'ZeptoClaw',
      description: 'Ultra-lightweight AI agent framework. Streaming, swarms, plugins — all in ~5MB of Rust.',
      logo: {
        light: './src/assets/logo.svg',
        dark: './src/assets/logo.svg',
        replacesTitle: true,
      },
      social: {
        github: 'https://github.com/qhkm/zeptoclaw',
      },
      sidebar: [
        {
          label: 'Getting Started',
          items: [
            { label: 'Introduction', link: '/docs/getting-started/introduction/' },
            { label: 'Installation', link: '/docs/getting-started/installation/' },
            { label: 'Quick Start', link: '/docs/getting-started/quick-start/' },
          ],
        },
        {
          label: 'Core Concepts',
          items: [
            { label: 'Agent Loop', link: '/docs/concepts/agent-loop/' },
            { label: 'Tools', link: '/docs/concepts/tools/' },
            { label: 'Channels', link: '/docs/concepts/channels/' },
            { label: 'Providers', link: '/docs/concepts/providers/' },
          ],
        },
        {
          label: 'Reference',
          items: [
            { label: 'CLI', link: '/docs/reference/cli/' },
            { label: 'Configuration', link: '/docs/reference/configuration/' },
            { label: 'Environment Variables', link: '/docs/reference/environment/' },
            { label: 'Tools Reference', link: '/docs/reference/tools/' },
          ],
        },
        {
          label: 'Guides',
          items: [
            { label: 'Plugins', link: '/docs/guides/plugins/' },
            { label: 'Agent Templates', link: '/docs/guides/templates/' },
            { label: 'Deployment', link: '/docs/guides/deployment/' },
            { label: 'Security', link: '/docs/guides/security/' },
          ],
        },
      ],
      customCss: ['./src/styles/custom.css'],
      favicon: '/favicon.svg',
      head: [
        {
          tag: 'meta',
          attrs: { property: 'og:image', content: '/og-image.png' },
        },
      ],
    }),
  ],
  site: 'https://zeptoclaw.pages.dev',
  base: '/docs',
});
