package dev.madhyam

import android.app.ActivityManager
import android.app.Application
import android.content.ComponentCallbacks2
import coil.ImageLoader
import coil.ImageLoaderFactory
import coil.imageLoader
import coil.memory.MemoryCache

class App : Application(), ImageLoaderFactory {
    /** One catalog for the whole process. */
    val repo: Repository by lazy { Repository(this) }

    /** Under 2 GB of RAM (most TVs and cheap boxes): every megabyte the app holds counts. */
    private val lowMemory: Boolean by lazy {
        val am = getSystemService(ACTIVITY_SERVICE) as ActivityManager
        val info = ActivityManager.MemoryInfo().also { am.getMemoryInfo(it) }
        am.isLowRamDevice || info.totalMem < 2L * 1024 * 1024 * 1024
    }

    /**
     * Posters and logos: a small memory cache (the disk cache keeps them for the next visit), and
     * 16-bit pixels for pictures without transparency on low-memory TVs (half the memory).
     */
    override fun newImageLoader(): ImageLoader = ImageLoader.Builder(this)
        .memoryCache { MemoryCache.Builder(this).maxSizePercent(if (lowMemory) 0.12 else 0.2).build() }
        .allowRgb565(lowMemory)
        .build()

    @Suppress("DEPRECATION") // the levels below UI_HIDDEN still arrive on the Android 5–13 TVs we support
    override fun onTrimMemory(level: Int) {
        super.onTrimMemory(level)
        // In the background or short of memory: drop decoded pictures before Android drops the app.
        if (level >= ComponentCallbacks2.TRIM_MEMORY_UI_HIDDEN || level == ComponentCallbacks2.TRIM_MEMORY_RUNNING_CRITICAL) {
            imageLoader.memoryCache?.clear()
        }
    }
}
