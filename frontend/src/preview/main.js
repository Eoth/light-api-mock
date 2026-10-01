// Entry of the design-system preview (design-preview.html, `npm run design:preview`). Vite serves it in development
// only: the build reads index.html alone, so nothing of the preview reaches the binary.
import { mount } from 'svelte';
import DesignPreview from './DesignPreview.svelte';
import '../tokens.css';
import '../app.css';

mount(DesignPreview, { target: document.getElementById('preview') });
