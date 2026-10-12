# JNA looks up classes and methods reflectively; R8 must not rename or strip them.
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
-dontwarn java.awt.**
-dontwarn com.sun.jna.**
# Our JNA interface to the Rust library.
-keep class dev.madhyam.core.** { *; }
