package com.lanvideo.player.ui.navigation

import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Scaffold
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.navigation.NavDestination.Companion.hierarchy
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import com.lanvideo.player.ui.auth.LoginScreen
import com.lanvideo.player.ui.components.BottomNavBar
import com.lanvideo.player.ui.home.HomeScreen
import com.lanvideo.player.ui.player.PlayerScreen
import com.lanvideo.player.ui.search.SearchScreen
import com.lanvideo.player.ui.user.UserScreen
import com.lanvideo.player.ui.history.HistoryScreen
import com.lanvideo.player.ui.settings.SettingsScreen
import com.lanvideo.player.ui.viewer.ImageViewerScreen
import com.lanvideo.player.ui.upload.UploadScreen

private fun encodeImageUrls(urls: List<String>): String {
    return urls.joinToString(",")
}

private fun decodeImageUrls(encoded: String): List<String> {
    return encoded.split(",").filter { it.isNotBlank() }
}

sealed class Screen(val route: String) {
    data object Home : Screen("home")
    data object Search : Screen("search")
    data object User : Screen("user")
    data object Player : Screen("player/{videoId}") {
        fun createRoute(videoId: String) = "player/$videoId"
    }
    data object History : Screen("history")
    data object Settings : Screen("settings")
    data object Login : Screen("login")
    data object ImageViewer : Screen("viewer/{imageUrls}/{startIndex}") {
        fun createRoute(imageUrls: String, startIndex: Int): String {
            val encoded = java.net.URLEncoder.encode(imageUrls, "UTF-8")
            return "viewer/$encoded/$startIndex"
        }
    }
    data object Upload : Screen("upload")
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
            modifier = Modifier.padding(innerPadding),
            enterTransition = { EnterTransition.None },
            exitTransition = { ExitTransition.None }
        ) {
            composable(Screen.Home.route) {
                HomeScreen(
                    onVideoClick = { videoId ->
                        navController.navigate(Screen.Player.createRoute(videoId))
                    },
                    onImageClick = { imageUrls, startIndex ->
                        val encoded = encodeImageUrls(imageUrls)
                        navController.navigate(Screen.ImageViewer.createRoute(encoded, startIndex))
                    }
                )
            }
            composable(Screen.Search.route) {
                SearchScreen(
                    onVideoClick = { videoId ->
                        navController.navigate(Screen.Player.createRoute(videoId))
                    },
                    onImageClick = { imageUrls, startIndex ->
                        val encoded = encodeImageUrls(imageUrls)
                        navController.navigate(Screen.ImageViewer.createRoute(encoded, startIndex))
                    }
                )
            }
            composable(Screen.User.route) {
                UserScreen(
                    onHistoryClick = { navController.navigate(Screen.History.route) },
                    onSettingsClick = { navController.navigate(Screen.Settings.route) },
                    onLoginClick = { navController.navigate(Screen.Login.route) },
                    onUploadClick = { navController.navigate(Screen.Upload.route) }
                )
            }
            composable(Screen.Player.route) { backStackEntry ->
                val videoId = backStackEntry.arguments?.getString("videoId") ?: ""
                PlayerScreen(
                    videoId = videoId,
                    onBackClick = { navController.popBackStack() },
                    onVideoClick = { id ->
                        navController.navigate(Screen.Player.createRoute(id)) {
                            popUpTo(Screen.Player.route) { inclusive = true }
                        }
                    }
                )
            }
            composable(Screen.History.route) {
                HistoryScreen(
                    onBackClick = { navController.popBackStack() }
                )
            }
            composable(Screen.Settings.route) {
                SettingsScreen()
            }
            composable(Screen.Login.route) {
                LoginScreen(
                    onLoginSuccess = {
                        navController.popBackStack()
                    },
                    onBack = {
                        navController.popBackStack()
                    }
                )
            }
            composable(
                route = Screen.ImageViewer.route,
                arguments = listOf(
                    navArgument("imageUrls") { type = NavType.StringType },
                    navArgument("startIndex") { type = NavType.IntType; defaultValue = 0 }
                )
            ) { backStackEntry ->
                val encodedUrls = backStackEntry.arguments?.getString("imageUrls") ?: ""
                val startIndex = backStackEntry.arguments?.getInt("startIndex") ?: 0
                val decodedUrls = try {
                    java.net.URLDecoder.decode(encodedUrls, "UTF-8")
                } catch (e: Exception) {
                    encodedUrls
                }
                val imageUrls = decodeImageUrls(decodedUrls)
                ImageViewerScreen(
                    imageUrls = imageUrls,
                    startIndex = startIndex,
                    onBackClick = { navController.popBackStack() }
                )
            }
            composable(Screen.Upload.route) {
                UploadScreen(
                    onBackClick = { navController.popBackStack() }
                )
            }
        }
    }
}
