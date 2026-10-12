package dev.madhyam

import android.app.Application

class App : Application() {
    /** One catalog for the whole process. */
    val repo: Repository by lazy { Repository(this) }
}
