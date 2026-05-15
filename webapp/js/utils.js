// =================================================================
// UTILS — shared helper functions
// =================================================================
export const $ = (s) => document.querySelector(s);
export const $$ = (s) => document.querySelectorAll(s);

export function gridItemById(id, container) {
    return (container || document).querySelector(`.grid-item[data-id="${CSS.escape(String(id))}"]`);
}
