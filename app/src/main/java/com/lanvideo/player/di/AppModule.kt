package com.lanvideo.player.di

import com.lanvideo.player.data.local.AppDatabase
import com.lanvideo.player.data.repository.VideoRepository
import com.lanvideo.player.ui.auth.LoginViewModel
import com.lanvideo.player.ui.history.HistoryViewModel
import com.lanvideo.player.ui.home.HomeViewModel
import com.lanvideo.player.ui.player.PlayerViewModel
import com.lanvideo.player.ui.search.SearchViewModel
import com.lanvideo.player.ui.user.UserViewModel
import org.koin.android.ext.koin.androidContext
import org.koin.androidx.viewmodel.dsl.viewModel
import org.koin.dsl.module

val appModule = module {

    // Room database (singleton)
    single { AppDatabase.getInstance(androidContext()) }

    // VideoRepository — now a class with injected database
    single { VideoRepository(get()) }

    // ViewModels
    viewModel { HomeViewModel(get()) }
    viewModel { SearchViewModel(get()) }
    viewModel { PlayerViewModel(get()) }
    viewModel { LoginViewModel(androidContext()) }
    viewModel { HistoryViewModel(androidContext()) }
    viewModel { UserViewModel(androidContext()) }
}
