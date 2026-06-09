---
name: ark-ui
description: Headless React component guidance for Ark UI. Use when building UI components with @ark-ui/react, accessible form inputs, overlays, navigation patterns, data attributes, asChild composition, or Ark state hooks.
---

# Ark UI

Ark UI is a headless component library that provides accessible, unstyled React primitives for custom UI components with full control over styling and behavior.

Use this skill when creating or changing components based on `@ark-ui/react`, especially form inputs, overlays, navigation components, and components that rely on Ark data attributes, anatomy, `asChild`, or state hooks.

## Key Patterns

- Use the root pattern, such as `Slider.Root`, `Slider.Track`, and related slot components.
- Style slots through Panda recipes and Ark anatomy where possible.
- Use data attributes for state-aware styling, such as `[data-state="open"]` and `[data-state="checked"]`.
- Use `asChild` to render a custom element while keeping Ark behavior.
- Use `Component.Context` or hooks such as `useAccordion()` when component state must be read or controlled.
- Prefer Ark-provided hidden inputs for form controls that need native form participation.

## Core Docs

Fetch the relevant Ark UI docs before broad or unfamiliar component work:

| Topic                     | URL                                                    |
| ------------------------- | ------------------------------------------------------ |
| Getting Started           | https://ark-ui.com/react/docs/overview/getting-started |
| Styling (data attributes) | https://ark-ui.com/react/docs/guides/styling           |
| Composition (`asChild`)   | https://ark-ui.com/react/docs/guides/composition       |
| Component State           | https://ark-ui.com/react/docs/guides/component-state   |
| Animation                 | https://ark-ui.com/react/docs/guides/animation         |
| Forms Integration         | https://ark-ui.com/react/docs/guides/forms             |
| Refs                      | https://ark-ui.com/react/docs/guides/ref               |
| Closed Components         | https://ark-ui.com/react/docs/guides/closed-components |

## Component Docs

### Form Inputs

| Component      | URL                                                     |
| -------------- | ------------------------------------------------------- |
| Checkbox       | https://ark-ui.com/react/docs/components/checkbox       |
| Combobox       | https://ark-ui.com/react/docs/components/combobox       |
| Color Picker   | https://ark-ui.com/react/docs/components/color-picker   |
| Date Picker    | https://ark-ui.com/react/docs/components/date-picker    |
| Editable       | https://ark-ui.com/react/docs/components/editable       |
| Field          | https://ark-ui.com/react/docs/components/field          |
| Fieldset       | https://ark-ui.com/react/docs/components/fieldset       |
| File Upload    | https://ark-ui.com/react/docs/components/file-upload    |
| Listbox        | https://ark-ui.com/react/docs/components/listbox        |
| Number Input   | https://ark-ui.com/react/docs/components/number-input   |
| Password Input | https://ark-ui.com/react/docs/components/password-input |
| Pin Input      | https://ark-ui.com/react/docs/components/pin-input      |
| Radio Group    | https://ark-ui.com/react/docs/components/radio-group    |
| Select         | https://ark-ui.com/react/docs/components/select         |
| Slider         | https://ark-ui.com/react/docs/components/slider         |
| Switch         | https://ark-ui.com/react/docs/components/switch         |
| Tags Input     | https://ark-ui.com/react/docs/components/tags-input     |

### Overlays and Popups

| Component      | URL                                                     |
| -------------- | ------------------------------------------------------- |
| Dialog         | https://ark-ui.com/react/docs/components/dialog         |
| Floating Panel | https://ark-ui.com/react/docs/components/floating-panel |
| Hover Card     | https://ark-ui.com/react/docs/components/hover-card     |
| Menu           | https://ark-ui.com/react/docs/components/menu           |
| Popover        | https://ark-ui.com/react/docs/components/popover        |
| Toast          | https://ark-ui.com/react/docs/components/toast          |
| Tooltip        | https://ark-ui.com/react/docs/components/tooltip        |
| Tour           | https://ark-ui.com/react/docs/components/tour           |

### Layout and Navigation

| Component   | URL                                                  |
| ----------- | ---------------------------------------------------- |
| Accordion   | https://ark-ui.com/react/docs/components/accordion   |
| Carousel    | https://ark-ui.com/react/docs/components/carousel    |
| Collapsible | https://ark-ui.com/react/docs/components/collapsible |
| Pagination  | https://ark-ui.com/react/docs/components/pagination  |
| Scroll Area | https://ark-ui.com/react/docs/components/scroll-area |
| Splitter    | https://ark-ui.com/react/docs/components/splitter    |
| Steps       | https://ark-ui.com/react/docs/components/steps       |
| Tabs        | https://ark-ui.com/react/docs/components/tabs        |
| Tree View   | https://ark-ui.com/react/docs/components/tree-view   |

## Project Patterns

- Place reusable Ark wrappers in `src/ui/primitives/` unless they are higher-level pure UI composites or layouts.
- Keep `src/ui/` components free of business logic and app-specific imports.
- Wrap Ark slots with `createStyleContext(recipe)` and `withProvider`, `withRootProvider`, or `withContext` to connect them to Panda slot recipes.
- Use anatomy keys from `@ark-ui/react/anatomy` when defining matching slot recipes in `src/theme/recipes/`.
- Export useful Ark context types and providers from the primitive when callers need controlled composition.
- For overlays that should not render until opened, follow existing primitives by setting `lazyMount` and `unmountOnExit` defaults when appropriate.
- Use the `panda-css` skill for styling, recipe variants, tokens, and static extraction details.
- Add or update a Storybook story for new or changed design-system components.

## Checklist

- [ ] Existing `src/ui` primitives were checked before creating a new wrapper.
- [ ] The selected Ark component docs were consulted for unfamiliar APIs.
- [ ] Slots are wired through the matching Panda recipe and anatomy names.
- [ ] Accessibility-critical pieces such as labels, hidden inputs, triggers, and focus behavior are preserved.
- [ ] `asChild` is used only when the child element can safely receive Ark props and refs.
- [ ] Business logic and feature-specific state stay outside `src/ui/`.
