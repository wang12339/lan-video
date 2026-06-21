package com.lanvideo.player.ui.navigation

import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Scaffold
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.navigation.NavDestination.Companion.hierarchy
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.lanvideo.player.ui.components.BottomNavBar
import com.lanvideo.player.ui.home.HomeScreen
import com.lanvideo.player.ui.player.PlayerScreen
import com.lanvideo.player.ui.search.SearchScreen
import com.lanvideo.player.ui.user.UserScreen
import com.lanvideo.player.ui.viewer.ImageViewerScreen

sealed class Screen(val route: String) {
    data object Home : Screen("home")
    data object Search : Screen("search")
    data object User : Screen("user")
    data object Player : Screen("player/{videoId}") {
        fun createRoute(videoId: String) = "player/$videoId"
    }
    data object History : Screen("history")
    data object Settings : Screen("settings")
    data object ImageViewer : Screen("viewer/{imageUrl}") {
        fun createRoute(imageUrl: String) = "viewer/$imageUrl"
    }
}

@Composable
fun AppNavigation() {
    val navController = rememberNavController()
    val navBackStackEntry by navController.currentBackStackEntryAsState()
    val currentDestination = navBackStackEntry?.destination

    val bottomBarScreens = listOf(Screen.Home, Screen.Search, Screen.User)
    val showBottomBar = currentDestination?.route in bottomBarScreens.map { it.route }

    Scaffold(
        bottomBar = {
            if (showBottomBar) {
                BottomNavBar(
                    currentRoute = currentDestination?.route,
                    onNavigate = { screen ->
                        navController.navigate(screen.route) {
                            popUpTo(navController.graph.findStartDestination().id) {
                                saveState = true
                            }
                            launchSingleTop = true
                            restoreState = true
                        }
                    }
                )
            }
        }
    ) { innerPadding ->
        NavHost(
            navController = navController,
            startDestination = Screen.Home.route,
            modifier = Modifier.padding(innerPadding)
        ) {
            composable(Screen.Home.route) {
                HomeScreen(
                    onVideoClick = { videoId ->
                        navController.navigate(Screen.Player.createRoute(videoId))
                    }
                )
            }
            composable(Screen.Search.route) {
                SearchScreen(
                    onVideoClick = { videoId ->
                        navController.navigate(Screen.Player.createRoute(videoId))
                    }
                )
            }
            composable(Screen.User.route) {
                UserScreen(
                    onHistoryClick = { navController.navigate(Screen.History.route) },
                    onSettingsClick = { navController.navigate(Screen.Settings.route) }
                )
            }
            composable(Screen.Player.route) { backStackEntry ->
                val videoId = backStackEntry.arguments?.getString("videoId") ?: ""
                PlayerScreen(
                    videoId = videoId,
                    onBackClick = { navController.popBackStack() }
                )
            }
            composable(Screen.History.route) {
            }
            composable(Screen.Settings.route) {
            }
            composable(Screen.ImageViewer.route) { backStackEntry ->
                val imageUrl = backStackEntry.arguments?.getString("imageUrl") ?: ""
                ImageViewerScreen(
                    imageUrl = imageUrl,
                    onBackClick = { navController.popBackStack() }
                )
            }
        }
    }
}
