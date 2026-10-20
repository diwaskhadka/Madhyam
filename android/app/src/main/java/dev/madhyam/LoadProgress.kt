package dev.madhyam

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

/**
 * Where a playlist load is: the step, and a detail such as the size downloaded so far. A
 * [final] status is a finished step's result (how many movies came in), kept on its line.
 */
data class LoadStatus(val step: Step, val detail: String = "", val final: Boolean = false) {
    enum class Step { SignIn, Live, Movies, Series, Download, Read, Save, Guide, Pages }

    companion object {
        fun size(bytes: Long): String = when {
            bytes >= 1L shl 30 -> "%.1f GB".format(bytes / (1L shl 30).toDouble())
            bytes >= 1L shl 20 -> "%.1f MB".format(bytes / (1L shl 20).toDouble())
            else -> "${maxOf(1, bytes / 1024)} KB"
        }

        val XTREAM = listOf(
            Step.SignIn to "Signing in", Step.Live to "Live channels", Step.Movies to "Movies", Step.Series to "Series",
            Step.Save to "Saving the catalog", Step.Guide to "Programme guide", Step.Pages to "Getting the pages ready",
        )
        val M3U = listOf(
            Step.Download to "Downloading the playlist", Step.Read to "Reading the playlist",
            Step.Live to "Live channels", Step.Movies to "Movies", Step.Series to "Series", Step.Save to "Saving the catalog",
            Step.Guide to "Programme guide", Step.Pages to "Getting the pages ready",
        )
    }
}

/** One line of the loading pop-up. */
class LoadLine(val step: LoadStatus.Step, val label: String) {
    var state by mutableStateOf(State.Waiting)
    var detail by mutableStateOf("")

    enum class State { Waiting, Running, Done, Failed }
}

/**
 * The pop-up shown while a playlist is added ([dev.madhyam.ui.LoadingDialog]): its lines, the
 * overall title, and the error if it failed. Fed by [report] from any thread.
 */
class LoadState(name: String, xtream: Boolean) {
    val lines = mutableStateListOf<LoadLine>().apply { (if (xtream) LoadStatus.XTREAM else LoadStatus.M3U).forEach { add(LoadLine(it.first, it.second)) } }
    var title by mutableStateOf("Adding “$name”")
    var error by mutableStateOf<String?>(null)
    var finished by mutableStateOf(false)
    /** Index of the running line, -1 before the first report. */
    var current by mutableStateOf(-1)
    val started = System.currentTimeMillis()
    var stepStarted by mutableStateOf(System.currentTimeMillis())

    fun report(s: LoadStatus) {
        if (finished) return
        val i = lines.indexOfFirst { it.step == s.step }
        if (i < 0) return
        if (i > current) {
            for (j in maxOf(0, current) until i) if (lines[j].state != LoadLine.State.Done) lines[j].state = LoadLine.State.Done
            current = i
            stepStarted = System.currentTimeMillis()
            lines[i].state = LoadLine.State.Running
        }
        if (s.final) {
            lines[i].state = LoadLine.State.Done
            lines[i].detail = s.detail
        } else if (i == current) {
            lines[i].detail = s.detail
        }
    }

    fun done() {
        report(LoadStatus(lines.last().step))
        lines.forEach { if (it.state != LoadLine.State.Done) it.state = LoadLine.State.Done }
        val took = (System.currentTimeMillis() - started) / 1000
        title = if (took >= 1) "Ready in $took s" else "Ready"
        finished = true
    }

    fun failed(message: String) {
        if (current >= 0) lines[current].state = LoadLine.State.Failed
        title = "The playlist could not be added"
        error = message
        finished = true
    }
}
