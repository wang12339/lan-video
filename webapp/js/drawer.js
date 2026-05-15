// =================================================================
// DRAWER — side navigation menu
// =================================================================
import { Dom } from './dom.js';

export function openDrawer() { Dom.drawer.classList.add('open'); Dom.drawerOverlay.classList.remove('hidden'); }
export function closeDrawer() { Dom.drawer.classList.remove('open'); Dom.drawerOverlay.classList.add('hidden'); }
