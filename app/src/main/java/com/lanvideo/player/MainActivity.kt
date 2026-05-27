package com.lanvideo.player

import android.content.Intent
import android.graphics.Color
import android.net.Uri
import android.os.Bundle
import android.view.Menu
import android.view.MenuItem
import androidx.activity.OnBackPressedCallback
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.GravityCompat
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.lifecycle.lifecycleScope
import androidx.navigation.NavController
import androidx.navigation.findNavController
import androidx.navigation.fragment.NavHostFragment
import androidx.navigation.ui.AppBarConfiguration
import androidx.navigation.ui.NavigationUI
import androidx.navigation.ui.navigateUp
import androidx.navigation.ui.setupActionBarWithNavController
import androidx.navigation.ui.setupWithNavController
import com.google.android.material.navigation.NavigationView
import com.lanvideo.player.data.network.LanConnectionManager
import com.lanvideo.player.data.repository.VideoRepository
import com.lanvideo.player.data.user.AuthSessionStore
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import com.lanvideo.player.feature.home.HomeFragment
import com.lanvideo.player.feature.player.PlayerPageFragment
import com.lanvideo.player.ui.auth.LoginDialog
import com.lanvideo.player.databinding.ActivityMainBinding

class MainActivity : AppCompatActivity() {

    private lateinit var appBarConfiguration: AppBarConfiguration
    private lateinit var binding: ActivityMainBinding

    private val videoRepository get() = VideoRepository

    private val pickFolder = registerForActivityResult(
        ActivityResultContracts.OpenDocumentTree()
    ) { uri: Uri? ->
        if (uri != null) {
            // 持久的读取权限，后续上传时还能访问
            val flags = Intent.FLAG_GRANT_READ_URI_PERMISSION
            contentResolver.takePersistableUriPermission(uri, flags)

            // 后台遍历文件夹，收集视频/图片文件
            lifecycleScope.launch(Dispatchers.IO) {
                val uris = traverseFolder(uri)
                if (uris.isNotEmpty()) {
                    withContext(Dispatchers.Main) {
                        findNavController(R.id.nav_host_fragment_content_main)
                            .navigate(R.id.nav_upload_list, Bundle().apply {
                                putParcelableArrayList("pendingUris", ArrayList(uris))
                            })
                    }
                }
            }
        }
    }

    private fun traverseFolder(treeUri: Uri): List<Uri> {
        val result = mutableListOf<Uri>()
        val root = androidx.documentfile.provider.DocumentFile.fromTreeUri(this, treeUri)
            ?: return result
        val children = root.listFiles()
        for (child in children) {
            if (child.isDirectory) {
                // 递归一层子目录
                val subChildren = child.listFiles()
                for (sub in subChildren) {
                    if (sub.isFile) {
                        val mime = sub.type ?: continue
                        if (mime.startsWith("video/") || mime.startsWith("image/"))
                            result.add(sub.uri)
                    }
                }
            } else if (child.isFile) {
                val mime = child.type ?: continue
                if (mime.startsWith("video/") || mime.startsWith("image/"))
                    result.add(child.uri)
            }
        }
        return result
    }

    fun openDrawer() {
        val drawer = binding.drawerLayout ?: return
        if (!drawer.isDrawerOpen(GravityCompat.START)) {
            drawer.openDrawer(GravityCompat.START)
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        binding = ActivityMainBinding.inflate(layoutInflater)
        setContentView(binding.root)
        WindowCompat.setDecorFitsSystemWindows(window, false)
        window.statusBarColor = Color.TRANSPARENT
        window.navigationBarColor = Color.TRANSPARENT

        LanConnectionManager.start(this)

        val navHostFragment =
            supportFragmentManager.findFragmentById(R.id.nav_host_fragment_content_main) as NavHostFragment
        val navController = navHostFragment.navController

        // Bottom nav setup
        binding.appBarMain.contentMain.bottomNavView?.let { bottomNav ->
            bottomNav.setupWithNavController(navController)
            // Handle navigation bar insets so bottom nav isn't covered
            ViewCompat.setOnApplyWindowInsetsListener(bottomNav) { v, insets ->
                val navBar = insets.getInsets(WindowInsetsCompat.Type.navigationBars())
                v.setPadding(v.paddingStart, v.paddingTop, v.paddingEnd, navBar.bottom)
                insets
            }
        }

        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                val navHost = supportFragmentManager.findFragmentById(R.id.nav_host_fragment_content_main)
                val homeFrag = navHost?.childFragmentManager?.fragments?.firstOrNull() as? HomeFragment
                if (homeFrag?.onBackPressed() == true) return
                isEnabled = false
                onBackPressedDispatcher.onBackPressed()
                isEnabled = true
            }
        })

        binding.navView?.let { navView ->
            navView.setNavigationItemSelectedListener { item ->
                if (item.itemId == R.id.nav_upload) {
                    pickFolder.launch(null)
                    binding.drawerLayout?.closeDrawer(GravityCompat.START)
                } else if (item.itemId == R.id.nav_upload_list) {
                    binding.drawerLayout?.closeDrawer(GravityCompat.START)
                    findNavController(R.id.nav_host_fragment_content_main)
                        .navigate(R.id.nav_upload_list)
                } else if (item.itemId == R.id.nav_history) {
                    binding.drawerLayout?.closeDrawer(GravityCompat.START)
                    findNavController(R.id.nav_host_fragment_content_main)
                        .navigate(R.id.nav_history)
                } else if (item.itemId == R.id.nav_batch_delete) {
                    navController.navigate(R.id.nav_home)
                    binding.drawerLayout?.closeDrawer(GravityCompat.START)
                    // 等 nav 切换完成后再触发
                    binding.root.postDelayed({
                        (application as MyApplication).setBatchDeleteRequested(true)
                    }, 300)
                } else {
                    if (NavigationUI.onNavDestinationSelected(item, navController)) {
                        binding.drawerLayout?.closeDrawer(GravityCompat.START)
                    }
                }
                true
            }
            navController.addOnDestinationChangedListener { _, dest, _ ->
                when (dest.id) {
                    R.id.nav_home -> navView.setCheckedItem(R.id.nav_home)
                    R.id.nav_search -> navView.setCheckedItem(R.id.nav_search)
                }
            }
        }
        // ── 访问密码门禁 ──
        if (savedInstanceState == null && !AuthSessionStore.isLoggedIn(this)) {
            LoginDialog().show(supportFragmentManager, "login")
        }
        // ── 401 token 过期时重新弹出登录页 ──
        (application as MyApplication).unauthorizedEvent.observe(this) { event ->
            if (event == true) {
                (application as MyApplication).clearUnauthorizedEvent()
                if (supportFragmentManager.findFragmentByTag("login") == null) {
                    AuthSessionStore.clear(this)
                    LoginDialog().show(supportFragmentManager, "login")
                }
            }
        }
    }

    // ── Picture-in-Picture: when user leaves app, try PiP on the player ──
    override fun onUserLeaveHint() {
        super.onUserLeaveHint()
        val navHost = supportFragmentManager.findFragmentById(R.id.nav_host_fragment_content_main) as? NavHostFragment
        val playerFrag = navHost?.childFragmentManager?.fragments?.firstOrNull()
            ?.childFragmentManager?.fragments?.firstOrNull() as? PlayerPageFragment
        playerFrag?.requestEnterPip()
    }

    override fun onCreateOptionsMenu(menu: Menu): Boolean {
        val result = super.onCreateOptionsMenu(menu)
        // Using findViewById because NavigationView exists in different layout files
        // between w600dp and w1240dp
        val navView: NavigationView? = findViewById(R.id.nav_view)
        if (navView == null) {
            // The navigation drawer already has the items including the items in the overflow menu
            // We only inflate the overflow menu if the navigation drawer isn't visible
            menuInflater.inflate(R.menu.overflow, menu)
        }
        return result
    }

    override fun onOptionsItemSelected(item: MenuItem): Boolean {
        return super.onOptionsItemSelected(item)
    }

    override fun onSupportNavigateUp(): Boolean {
        val navController = findNavController(R.id.nav_host_fragment_content_main)
        return navController.navigateUp(appBarConfiguration) || super.onSupportNavigateUp()
    }

    override fun onDestroy() {
        LanConnectionManager.stop(this)
        super.onDestroy()
    }
}