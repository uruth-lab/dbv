# DBV - Data Builder Visualizer

This is a tool developed as part of Chester's Master's Thesis in service of other experiments.
It is published here for use by anyone for whom it provides value as is.
Feel free to create issues if you find problems or have feature requests but while you are likely to get an answer there is no guarantee that your request will be fulfilled.
However, if you wish to fork the code and have questions regarding how to go about making changes I will try to help where possible but due to time constraints it may not always be possible.

# Running the Tool

## Running in browser

To run the tool in your browser requires no installation.
It is hosted and available via github pages [here](https://uruth-lab.github.io/dbv/).

## Local installation

We presently do not (and do not plan to, unless requested) provide a compiled binary version of the application.
The code is freely available in this repo and can be compiled and installed on your machine if you wish.
To compile and install locally will require you to have [rust installed](https://www.rust-lang.org/tools/install) and any dependencies for your OS to be able to run egui/eframe applications.

### Possible dependencies

My expectation is that you will require similar dependencies to the [egui Demo](https://github.com/emilk/egui?tab=readme-ov-file#demo).
For ease of reference I've included them here from their readme so you can easily reference them as needed.
I haven't invested the time into setting up a clean machine to test these on and only provide them as a convenience.
Personally I would try to install without installing the dependencies listed below and see if they are actually needed.

> ... should work out-of-the-box on Mac and Windows, but on Linux you need to first run:

```sh
sudo apt-get install -y libclang-dev libgtk-3-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev
```

On Fedora Rawhide you need to run:

```sh
dnf install clang clang-devel clang-tools-extra libxkbcommon-devel pkg-config openssl-devel libxcb-devel gtk3-devel atk fontconfig-devel
```

### Installation steps

Once rust and any other dependencies (not sure there are any) have been taken care of you can use the following command to ask cargo to install the most recent version.

```sh
cargo install --git https://github.com/uruth-lab/dbv --locked
```

# Credits

- My professor for approving the development of this tool as part of my thesis. It turned out to be quite useful for quickly experimenting with ideas.
- This project is started from the [egui template](https://github.com/emilk/eframe_template/).
- It is heavily inspired by the plots part of the [egui demo](https://www.egui.rs/#demo).
- The good people in the [egui discord](https://discord.com/invite/JFcEma9bJq) that provided guidance and invaluable assistance.
- [Beixuan Yang](http://beixuanyang.com/) for her assistance with the math for the plot boundaries reset.

## License

All code in this repository is dual-licensed under either:

- Apache License, Version 2.0
- MIT license

at your option.
This means you can select the license you prefer!
This dual-licensing approach is the de-facto standard in the Rust ecosystem and there are very good reasons to include both as noted in
this [issue](https://github.com/bevyengine/bevy/issues/2373) on [Bevy](https://bevyengine.org)'s repo.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
