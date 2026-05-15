// =================================================================
// STATE — centralized application state
// =================================================================
export const State = {
    _data: {
        connectionStatus: 'scanning',
        homeTab: 0, isSelectMode: false,
        selectedIds: new Set(), allVideos: [], images: [],
        searchResults: [], searchQuery: '', searchPage: 0, searchTotal: 0,
        isLoadingMore: false, currentPage: 'home',
        playerVideoId: null, playerCategory: null,
        playerList: [], playerIndex: -1,
        imageList: [], imageIndex: 0,
        slideshowList: [], slideshowIndex: 0, slideshowPlaying: false,
    },
    get(k) { return this._data[k]; },
    set(k, v) { this._data[k] = v; },
};
