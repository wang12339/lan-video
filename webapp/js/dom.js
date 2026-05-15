// =================================================================
// DOM REFERENCES — cached element lookups + helper
// =================================================================
import { $, $$, gridItemById } from './utils.js';

export { gridItemById };

export const Dom = {
    pageHome: $('#pageHome'), pageSearch: $('#pageSearch'),
    pageSlideshow: $('#pageSlideshow'), pageSettings: $('#pageSettings'),
    pagePlayer: $('#pagePlayer'), pageImageViewer: $('#pageImageViewer'),
    allPages: () => $$('.page'), allFullPages: () => $$('.page-full'),
    pageTitle: $('#pageTitle'), btnMenu: $('#btnMenu'),
    drawer: $('#drawer'), drawerOverlay: $('#drawerOverlay'),
    drawerItems: () => $$('.drawer-item[data-page]'),
    bottomNav: $('#bottomNav'), navItems: () => $$('.nav-item'),
    connectionStatus: $('#connectionStatus'), statusDot: $('#statusDot'), statusText: $('#statusText'),
    tabs: () => $$('.tab'), tabIndicator: $('#tabIndicator'),
    grid: $('#grid'), skeletonGrid: $('#skeletonGrid'),
    loadingSpinner: $('#loadingSpinner'), emptyState: $('#emptyState'), emptyText: $('#emptyText'),
    selectionBar: $('#selectionBar'), selectionCount: $('#selectionCount'),
    btnSelectAll: $('#btnSelectAll'), btnCancelSelect: $('#btnCancelSelect'), btnDeleteSelected: $('#btnDeleteSelected'),
    searchInput: $('#searchInput'), searchGrid: $('#searchGrid'),
    searchEmpty: $('#searchEmpty'), searchEmptyText: $('#searchEmptyText'), searchLoadMore: $('#searchLoadMore'),
    videoPlayer: $('#videoPlayer'), playerTitle: $('#playerTitle'),
    speedIndicator: $('#speedIndicator'), btnPlayerBack: $('#btnPlayerBack'),
    btnPlayerFullscreen: $('#btnPlayerFullscreen'),
    imageDisplay: $('#imageDisplay'), imageTitle: $('#imageTitle'), imageCounter: $('#imageCounter'),
    btnImageBack: $('#btnImageBack'), btnImagePrev: $('#btnImagePrev'), btnImageNext: $('#btnImageNext'),
    pageSlideshowFull: $('#pageSlideshowFull'),
    slideshowGrid: $('#slideshowGrid'), slideshowEmpty: $('#slideshowEmpty'),
    slideshowDisplay: $('#slideshowDisplay'), slideshowTitle: $('#slideshowTitle'),
    slideshowCounter: $('#slideshowCounter'), slideshowProgressFill: $('#slideshowProgressFill'),
    btnSlideshowBack: $('#btnSlideshowBack'), btnSlideshowPrev: $('#btnSlideshowPrev'),
    btnSlideshowNext: $('#btnSlideshowNext'), btnSlideshowPlay: $('#btnSlideshowPlay'),
    inputServerUrl: $('#inputServerUrl'), inputAdminToken: $('#inputAdminToken'),
    btnSaveServer: $('#btnSaveServer'), btnSaveToken: $('#btnSaveToken'), serverVersion: $('#serverVersion'),
    accessGateOverlay: $('#accessGateOverlay'), accessPasswordInput: $('#accessPasswordInput'),
    accessPasswordError: $('#accessPasswordError'), btnAccessSubmit: $('#btnAccessSubmit'),
    uploadOverlay: $('#uploadOverlay'), uploadStatus: $('#uploadStatus'),
    uploadProgressFill: $('#uploadProgressFill'), uploadProgressText: $('#uploadProgressText'),
    filePicker: $('#filePicker'), folderPicker: $('#folderPicker'),
    drawerUpload: $('#drawerUpload'), drawerFolderUpload: $('#drawerFolderUpload'), drawerBatchDelete: $('#drawerBatchDelete'),
};
