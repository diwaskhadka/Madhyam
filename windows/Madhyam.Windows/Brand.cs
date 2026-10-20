using System.Reflection;

namespace Madhyam.Windows;

/// The app's name as users see it. It comes from branding/brand.ini through the project file
/// (Product, BrandNativeName), so the source code never spells it out.
public static class Brand
{
    public static string Name { get; } =
        typeof(Brand).Assembly.GetCustomAttribute<AssemblyProductAttribute>()?.Product ?? "Madhyam";

    public static string NativeName { get; } =
        typeof(Brand).Assembly.GetCustomAttributes<AssemblyMetadataAttribute>()
            .FirstOrDefault(a => a.Key == "BrandNativeName")?.Value ?? "";

    /// Both names, for places with room for them (About, the sidebar logo's tooltip).
    public static string FullName => NativeName.Length > 0 ? $"{Name} · {NativeName}" : Name;

    /// Settings › About › Buy me a coffee.
    public const string CoffeeUrl = "https://buymeacoffee.com/diwaskhadka";
}
