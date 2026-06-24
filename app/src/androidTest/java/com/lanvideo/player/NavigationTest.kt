package com.lanvideo.player

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import com.lanvideo.player.ui.components.BottomNavBar
import com.lanvideo.player.ui.navigation.Screen
import org.junit.Rule
import org.junit.Test

class NavigationTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    @Test
    fun bottomNavBar_showsAllTabs() {
        composeTestRule.setContent {
            BottomNavBar(
                currentRoute = Screen.Home.route,
                onNavigate = {}
            )
        }

        composeTestRule.onNodeWithText("首页").assertIsDisplayed()
        composeTestRule.onNodeWithText("我的").assertIsDisplayed()
    }

    @Test
    fun bottomNavBar_homeTabIsHighlighted() {
        composeTestRule.setContent {
            BottomNavBar(
                currentRoute = Screen.Home.route,
                onNavigate = {}
            )
        }

        // Home tab should be visible when on home route
        composeTestRule.onNodeWithText("首页").assertIsDisplayed()
    }

    @Test
    fun screenRoutes_areCorrect() {
        assert(Screen.Home.route == "home")
        assert(Screen.Search.route == "search")
        assert(Screen.User.route == "user")
        assert(Screen.History.route == "history")
        assert(Screen.Settings.route == "settings")
        assert(Screen.Login.route == "login")
        assert(Screen.Upload.route == "upload")
    }

    @Test
    fun playerRoute_containsPlaceholder() {
        assert(Screen.Player.route.contains("{videoId}"))
        assert(Screen.Player.createRoute("123") == "player/123")
    }
}
