//! The mechanical rule `worklist new` already documents: lowercase, common
//! Latin accents stripped, and anything outside `[a-z0-9]` collapsed to one
//! `-`. `new` ports it as-is — nothing here is muckpile's own.

use muckpile_core::slugify_title;

#[test]
fn lowercases_and_hyphenates_spaces() {
    assert_eq!(slugify_title("Arreglar el hook que no arranca"), "arreglar-el-hook-que-no-arranca");
}

#[test]
fn strips_leading_and_trailing_punctuation_without_a_stray_hyphen() {
    assert_eq!(slugify_title("¿el rol se hereda de la capa de arriba?"), "el-rol-se-hereda-de-la-capa-de-arriba");
}

#[test]
fn strips_common_latin_accents_instead_of_dropping_the_letter() {
    assert_eq!(slugify_title("Migración a categorías más específicas"), "migracion-a-categorias-mas-especificas");
}

#[test]
fn collapses_a_run_of_punctuation_to_a_single_hyphen() {
    assert_eq!(slugify_title("push: no pisa -- ni con --force"), "push-no-pisa-ni-con-force");
}

#[test]
fn has_no_length_cap() {
    let title = "Este es un titulo bastante largo que describe con detalle un problema especifico y no se recorta";
    let slug = slugify_title(title);
    assert_eq!(slug, "este-es-un-titulo-bastante-largo-que-describe-con-detalle-un-problema-especifico-y-no-se-recorta");
}
