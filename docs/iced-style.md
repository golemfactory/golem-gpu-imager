# Iced Styling Guide

## Dynamic Lists with keyed_column (todos example)

When dealing with lists where items can be added, removed, or reordered, using keyed_column is more efficient than a simple column. It helps Iced optimize rendering and state management by associating a unique key (like a Uuid) with each element.

```rust
// From examples/todos/src/main.rs  
fn view(&self) -> Element<Message> {  
    // ... other view setup ...

    let filtered_tasks = tasks.iter().filter(|task| filter.matches(task));

    let tasks_view: Element<_> = if filtered_tasks.clone().count() > 0 { // Use clone for count  
        keyed_column( // Use keyed_column instead of column!  
            tasks  
                .iter()  
                .enumerate()  
                .filter(|(_, task)| filter.matches(task))  
                .map(|(i, task)| {  
                    (  
                        task.id, // Unique key for each task  
                        task.view(i).map(Message::TaskMessage.with(i)),  
                    )  
                }),  
        )  
        .spacing(10)  
        .into()  
    } else {  
        // ... empty message ...  
    };

    // ... rest of view ...  
    column![title, input, controls, tasks_view] // Integrate the keyed_column  
        .spacing(20)  
        .max_width(800)  
        .into() // Explicit into() might be needed depending on context  
}
```

* **Key:** Each child element in keyed_column is a tuple (key, element). The key should uniquely identify the item (e.g., task.id).
* **Efficiency:** Iced uses these keys to efficiently update the UI when the list changes, avoiding unnecessary redraws of unchanged items.

## Custom Fonts for Icons (todos, editor, changelog examples)

Instead of using image files for small icons (like edit or delete buttons), you can use icon fonts. This is often more performant and scales better.

1. **Load the Font:** Load the .ttf font file (e.g., using include_bytes!) when initializing your application.  
   ```rust
   // From examples/todos/src/main.rs  
   iced::application(Todos::new, Todos::update, Todos::view)  
   // ... other settings ...  
   .font(Todos::ICON_FONT) // Load the icon font bytes  
   .run()
   ```

2. **Define Icon Helpers:** Create helper functions to generate Text widgets using the specific font and the correct Unicode codepoints for your icons.  
   ```rust
   // From examples/todos/src/main.rs  
   fn icon(unicode: char) -> Text<'static> {  
       text(unicode.to_string())  
           .font(Font::with_name("Iced-Todos-Icons")) // Specify the font name  
           .width(20)  
           .align_x(Center)  
   }

   fn edit_icon() -> Text<'static> {  
       icon('\u{F303}') // Unicode for the edit icon in this font  
   }

   fn delete_icon() -> Text<'static> {  
       icon('\u{F1F8}') // Unicode for the delete icon  
   }
   ```

3. **Use in View:** Use these helper functions within your view logic, often inside buttons.  
   ```rust
   // From examples/todos/src/main.rs (inside Task::view)  
   button(edit_icon()) // Use the helper function  
       .on_press(TaskMessage::Edit)  
       .padding(10)  
       .style(button::text) // Style the button appropriately
   ```

## Overlays for Modals and Tooltips (modal, tooltip, gallery examples)

For elements that need to appear *on top* of other UI elements (like modals, dropdowns, or tooltips), Iced uses an overlay system.

* **Modals (modal, gallery examples):** A common pattern is to use a stack! widget. The base layer is your main UI, and the top layer is the modal content, often wrapped in opaque and mouse_area to create a dimming effect and handle dismissal.  
  ```rust
  // Simplified from examples/modal/src/main.rs  
  fn modal<'a, Message>(  
      base: impl Into<Element<'a, Message>>,  
      content: impl Into<Element<'a, Message>>,  
      on_blur: Message,  
  ) -> Element<'a, Message>  
  where  
      Message: Clone + 'a,  
  {  
      stack![ // Layer 1: Base UI  
          base.into(),  
          // Layer 2: Modal overlay  
          opaque( // Make the background dimming effect  
              mouse_area( // Detect clicks outside the modal content  
                  center(opaque(content)) // Center the actual modal content  
                  .style(|_theme| { // Style the dimming background  
                      container::Style {  
                          background: Some(  
                              Color { a: 0.8, ..Color::BLACK }.into(),  
                          ),  
                          ..container::Style::default()  
                      }  
                  })  
              )  
              .on_press(on_blur) // Close modal on background click  
          )  
      ]  
      .into()  
  }

  // Usage in App::view  
  if self.show_modal {  
      modal(content, signup_form, Message::HideModal) // Wrap the main content  
  } else {  
      content.into()  
  }
  ```

* **Tooltips (tooltip example):** The tooltip widget handles the overlay logic for you. You provide the base widget, the tooltip content, and the desired position.  
  ```rust
  // From examples/tooltip/src/main.rs  
  let tooltip = tooltip( // The main widget  
      button("Press to change position")  
          .on_press(Message::ChangePosition),  
      // The content to show in the tooltip  
      position_to_text(self.position),  
      // Where to position the tooltip relative to the button  
      self.position,  
  )  
  .gap(10) // Space between button and tooltip  
  .style(container::rounded_box); // Style the tooltip container
  ```

## Gradient Backgrounds (gradient example)

Containers can use gradients for their background style.

```rust
// From examples/gradient/src/main.rs  
container(horizontal_space())  
    .style(move |_theme| {  
        let gradient = gradient::Linear::new(angle) // Define angle  
            .add_stop(0.0, start_color) // Add color stops  
            .add_stop(1.0, end_color);

        gradient.into() // Convert gradient to a Background  
    })  
    .width(Fill)  
    .height(Fill)
```

## Advanced Styling with Canvas (color_palette, clock examples)

For highly custom visuals, the Canvas widget provides a drawing API. You can draw shapes, paths, text, and images programmatically. This allows for very elegant and unique designs that go beyond standard widget styling.

* **Drawing Shapes and Text (clock example):**  
  ```rust
  // Simplified from examples/clock/src/main.rs  
  impl<Message> canvas::Program<Message> for Clock {  
      // ...  
      fn draw(/* ... */) -> Vec<Geometry> {  
          let clock = self.clock.draw(renderer, bounds.size(), |frame| {  
              let center = frame.center();  
              let radius = frame.width().min(frame.height()) / 2.0;  
              let palette = theme.extended_palette();

              // Draw background circle  
              let background = Path::circle(center, radius);  
              frame.fill(&background, palette.secondary.strong.color);

              // Define clock hands as paths  
              let short_hand = Path::line(Point::ORIGIN, Point::new(0.0, -0.5 * radius));  
              // ... define long_hand ...

              // Apply transformations (translate, rotate) and draw hands  
              frame.translate(Vector::new(center.x, center.y));  
              frame.with_save(|frame| {  
                  frame.rotate(/* calculate hour angle */);  
                  frame.stroke(&short_hand, /* stroke style */);  
              });  
              // ... draw minute and second hands ...

              // Draw numbers  
              for hour in 1..=12 {  
                  // ... calculate position based on angle ...  
                  frame.fill_text(canvas::Text {  
                      content: format!("{}", hour),  
                      position: /* calculated position */,  
                      color: palette.secondary.strong.text,  
                      // ... other text properties ...  
                  });  
              }  
          });  
          vec![clock]  
      }  
  }
  ```

* **Visualizing Data (color_palette example):** The canvas is used to draw the generated color palette swatches and corresponding hex codes.

## Grid Layout (gallery example)

For displaying items in a grid, the grid widget is useful. It can arrange items fluidly based on a target width.

```rust
// From examples/gallery/src/main.rs  
let images = self  
    .images  
    .iter()  
    .map(|image| card(image, self.previews.get(&image.id), self.now)) // card() returns an Element  
    .chain((self.images.len()..=Image::LIMIT).map(|_| placeholder())); // Add placeholders

let gallery = grid(images) // Pass iterator of Elements  
    .fluid(Preview::WIDTH) // Target width for each item  
    .height(grid::aspect_ratio(Preview::WIDTH, Preview::HEIGHT)) // Maintain aspect ratio  
    .spacing(10); // Spacing between grid items
```

By combining these layout primitives, styling options, theming, and occasionally the Canvas for custom drawing, you can create sophisticated and elegant user interfaces with Iced.